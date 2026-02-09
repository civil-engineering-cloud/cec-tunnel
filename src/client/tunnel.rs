//! 隧道客户端实现

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::common::protocol::{ClientInfo, TunnelConfig, TunnelInfo, WsMessage};

pub struct TunnelClient {
    server_url: String,
    client_info: ClientInfo,
    tunnel_configs: Vec<TunnelConfig>,
    tunnels: Arc<RwLock<HashMap<String, TunnelInfo>>>,
    connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>>,
}

impl TunnelClient {
    pub fn new(
        server: &str,
        name: &str,
        tunnel_strs: &[String],
        _token: Option<String>,
    ) -> Result<Self> {
        let hostname = hostname::get()?.to_string_lossy().to_string();

        let client_info = ClientInfo {
            id: String::new(),
            name: name.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            hostname,
            local_ip: get_local_ip(),
        };

        let tunnel_configs = tunnel_strs
            .iter()
            .filter_map(|t| TunnelConfig::parse(t))
            .collect();

        Ok(Self {
            server_url: server.to_string(),
            client_info,
            tunnel_configs,
            tunnels: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn run(&self) -> Result<()> {
        loop {
            match self.connect_and_run().await {
                Ok(_) => {
                    info!("连接已关闭，5秒后重连...");
                }
                Err(e) => {
                    error!("连接错误: {}，5秒后重连...", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn connect_and_run(&self) -> Result<()> {
        info!("正在连接 {}...", self.server_url);

        let (ws_stream, _) = connect_async(&self.server_url).await?;
        let (mut write, mut read) = ws_stream.split();

        info!("已连接到服务器");

        // 发送注册消息
        let register_msg = WsMessage::Register {
            client: self.client_info.clone(),
            tunnels: self.tunnel_configs.clone(),
        };
        let msg_text = serde_json::to_string(&register_msg)?;
        write.send(Message::Text(msg_text)).await?;

        // 创建发送通道
        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

        // 发送任务 — Data 用 Binary 帧，其他用 Text/JSON
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let ws_msg = match &msg {
                    WsMessage::Data { conn_id, data } => {
                        // Binary 帧: conn_id(36 bytes) + payload
                        let mut buf = Vec::with_capacity(36 + data.len());
                        let id_bytes = conn_id.as_bytes();
                        if id_bytes.len() >= 36 {
                            buf.extend_from_slice(&id_bytes[..36]);
                        } else {
                            buf.extend_from_slice(id_bytes);
                            buf.resize(36, 0);
                        }
                        buf.extend_from_slice(data);
                        Message::Binary(buf)
                    }
                    _ => {
                        match serde_json::to_string(&msg) {
                            Ok(t) => Message::Text(t),
                            Err(_) => continue,
                        }
                    }
                };
                if write.send(ws_msg).await.is_err() {
                    break;
                }
            }
        });

        // 心跳任务
        let tx_ping = tx.clone();
        let ping_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                let timestamp = chrono::Utc::now().timestamp();
                if tx_ping.send(WsMessage::Ping { timestamp }).is_err() {
                    break;
                }
            }
        });

        // 接收消息
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let ws_msg: WsMessage = match serde_json::from_str(&text) {
                        Ok(m) => m,
                        Err(e) => {
                            warn!("无效消息: {}", e);
                            continue;
                        }
                    };

                    match ws_msg {
                        WsMessage::RegisterResponse {
                            success,
                            client_id,
                            tunnels,
                            message,
                        } => {
                            if success {
                                info!("注册成功，客户端 ID: {}", client_id);
                                for tunnel in &tunnels {
                                    info!(
                                        "  隧道 {} -> {}:{} (服务端端口: {})",
                                        tunnel.name,
                                        tunnel.local_addr,
                                        tunnel.local_port,
                                        tunnel.server_port
                                    );
                                    let mut t = self.tunnels.write().await;
                                    t.insert(tunnel.id.clone(), tunnel.clone());
                                }
                            } else {
                                error!("注册失败: {:?}", message);
                                return Err(anyhow::anyhow!("注册失败"));
                            }
                        }
                        WsMessage::NewConnection { tunnel_id, conn_id } => {
                            debug!("新连接 {} (隧道 {})", conn_id, tunnel_id);
                            self.handle_new_connection(&tunnel_id, &conn_id, tx.clone())
                                .await;
                        }
                        WsMessage::Data { conn_id, data } => {
                            self.handle_data(&conn_id, data).await;
                        }
                        WsMessage::CloseConnection { conn_id } => {
                            self.handle_close(&conn_id).await;
                        }
                        WsMessage::Pong { .. } => {
                            debug!("收到 Pong");
                        }
                        WsMessage::Error { code, message } => {
                            error!("服务器错误 {}: {}", code, message);
                        }
                        WsMessage::AddTunnel { request_id, tunnel: config } => {
                            info!(
                                "服务端下发隧道: {}:{} -> 服务端端口 {:?}",
                                config.local_addr, config.local_port, config.remote_port
                            );
                            // 服务端已创建隧道，客户端只需记录本地映射
                            let tunnel_info = TunnelInfo {
                                id: request_id.clone(),
                                client_id: String::new(),
                                tunnel_type: config.tunnel_type.clone(),
                                name: config.name.clone().unwrap_or_default(),
                                local_addr: config.local_addr.clone(),
                                local_port: config.local_port,
                                server_port: config.remote_port.unwrap_or(0),
                                state: "active".to_string(),
                                bytes_sent: 0,
                                bytes_recv: 0,
                                created_at: String::new(),
                                last_active_at: String::new(),
                            };
                            let mut t = self.tunnels.write().await;
                            t.insert(tunnel_info.id.clone(), tunnel_info.clone());
                            info!(
                                "隧道已记录: {} -> {}:{} (服务端端口: {})",
                                tunnel_info.name, tunnel_info.local_addr,
                                tunnel_info.local_port, tunnel_info.server_port
                            );
                        }
                        WsMessage::AddTunnelResponse { request_id, success, tunnel, .. } => {
                            // 服务端确认隧道已创建，更新本地 tunnel 映射
                            if success {
                                if let Some(info) = tunnel {
                                    info!(
                                        "隧道已分配: {} -> {}:{} (服务端端口: {})",
                                        info.name, info.local_addr, info.local_port, info.server_port
                                    );
                                    let mut t = self.tunnels.write().await;
                                    t.insert(info.id.clone(), info);
                                }
                            } else {
                                warn!("隧道分配失败: request_id={}", request_id);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Message::Binary(data)) => {
                    // Binary 帧: conn_id(36 bytes) + payload
                    if data.len() > 36 {
                        let conn_id = String::from_utf8_lossy(&data[..36]).to_string();
                        let payload = data[36..].to_vec();
                        self.handle_data(&conn_id, payload).await;
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("服务器关闭连接");
                    break;
                }
                Err(e) => {
                    error!("WebSocket 错误: {}", e);
                    break;
                }
                _ => {}
            }
        }

        send_task.abort();
        ping_task.abort();

        Ok(())
    }

    async fn handle_new_connection(
        &self,
        tunnel_id: &str,
        conn_id: &str,
        tx: mpsc::UnboundedSender<WsMessage>,
    ) {
        let tunnels = self.tunnels.read().await;
        let tunnel = match tunnels.get(tunnel_id) {
            Some(t) => t.clone(),
            None => {
                warn!("未知隧道: {}", tunnel_id);
                return;
            }
        };
        drop(tunnels);

        let local_addr = format!("{}:{}", tunnel.local_addr, tunnel.local_port);
        let conn_id = conn_id.to_string();
        let tunnel_id = tunnel_id.to_string();
        let connections = Arc::clone(&self.connections);

        tokio::spawn(async move {
            // 连接本地服务
            let stream = match TcpStream::connect(&local_addr).await {
                Ok(s) => s,
                Err(e) => {
                    error!("连接本地服务 {} 失败: {}", local_addr, e);
                    let _ = tx.send(WsMessage::CloseConnection {
                        conn_id: conn_id.clone(),
                    });
                    return;
                }
            };

            debug!("已连接本地服务 {}", local_addr);

            // 通知服务端连接就绪
            let _ = tx.send(WsMessage::ConnectionReady {
                tunnel_id: tunnel_id.clone(),
                conn_id: conn_id.clone(),
            });

            // 创建数据通道
            let (data_tx, mut data_rx) = mpsc::unbounded_channel::<Vec<u8>>();
            {
                let mut conns = connections.write().await;
                conns.insert(conn_id.clone(), data_tx);
            }

            let (mut read_half, mut write_half) = stream.into_split();
            let conn_id_clone = conn_id.clone();
            let tx_clone = tx.clone();

            // 从本地服务读取，发送到服务端
            let read_task = tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                loop {
                    match read_half.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            let data = buf[..n].to_vec();
                            if tx_clone
                                .send(WsMessage::Data {
                                    conn_id: conn_id_clone.clone(),
                                    data,
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            // 从服务端接收，写入本地服务
            let write_task = tokio::spawn(async move {
                while let Some(data) = data_rx.recv().await {
                    if write_half.write_all(&data).await.is_err() {
                        break;
                    }
                }
            });

            tokio::select! {
                _ = read_task => {}
                _ = write_task => {}
            }

            // 清理
            {
                let mut conns = connections.write().await;
                conns.remove(&conn_id);
            }
            let _ = tx.send(WsMessage::CloseConnection { conn_id });
        });
    }

    async fn handle_data(&self, conn_id: &str, data: Vec<u8>) {
        let conns = self.connections.read().await;
        if let Some(tx) = conns.get(conn_id) {
            let _ = tx.send(data);
        }
    }

    async fn handle_close(&self, conn_id: &str) {
        let mut conns = self.connections.write().await;
        conns.remove(conn_id);
        debug!("连接 {} 已关闭", conn_id);
    }
}

fn get_local_ip() -> String {
    if let Ok(addrs) = local_ip_address::list_afinet_netifas() {
        for (_, ip) in addrs {
            if !ip.is_loopback() && ip.is_ipv4() {
                return ip.to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}
