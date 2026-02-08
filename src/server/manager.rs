//! 隧道管理器

use crate::common::protocol::{ClientInfo, TunnelConfig, TunnelInfo, WsMessage};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct ServerState {
    pub clients: Arc<DashMap<String, ClientState>>,
    pub tunnels: Arc<DashMap<String, TunnelState>>,
    pub connections: Arc<DashMap<String, ConnectionState>>,
    pub port_start: u16,
    pub port_end: u16,
    #[allow(dead_code)]
    pub auth_token: Option<String>,
}

pub struct ClientState {
    pub info: ClientInfo,
    #[allow(dead_code)] // 预留：服务端主动推送
    pub tx: mpsc::UnboundedSender<WsMessage>,
    pub tunnel_ids: Vec<String>,
}

pub struct TunnelState {
    pub info: TunnelInfo,
    pub shutdown: Option<tokio::sync::broadcast::Sender<()>>,
}

pub struct ConnectionState {
    #[allow(dead_code)] // 预留：连接追踪
    pub tunnel_id: String,
    pub client_id: String,
    pub tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl ServerState {
    pub fn new(port_start: u16, port_end: u16, auth_token: Option<String>) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            tunnels: Arc::new(DashMap::new()),
            connections: Arc::new(DashMap::new()),
            port_start,
            port_end,
            auth_token,
        }
    }

    pub async fn register_client(
        &self,
        client: ClientInfo,
        tunnels: Vec<TunnelConfig>,
        tx: mpsc::UnboundedSender<WsMessage>,
    ) -> Result<(String, Vec<TunnelInfo>), String> {
        let client_id = if client.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            client.id.clone()
        };
        let mut tunnel_infos = Vec::new();
        let mut tunnel_ids = Vec::new();

        for config in tunnels {
            match self.create_tunnel(&client_id, config, tx.clone()).await {
                Ok(info) => {
                    tunnel_ids.push(info.id.clone());
                    tunnel_infos.push(info);
                }
                Err(e) => {
                    warn!("创建隧道失败: {}", e);
                }
            }
        }

        let mut stored_client = client;
        stored_client.id = client_id.clone();

        self.clients.insert(
            client_id.clone(),
            ClientState {
                info: stored_client,
                tx,
                tunnel_ids,
            },
        );

        info!("客户端注册: {} ({} 个隧道)", client_id, tunnel_infos.len());
        Ok((client_id, tunnel_infos))
    }

    async fn create_tunnel(
        &self,
        client_id: &str,
        config: TunnelConfig,
        client_tx: mpsc::UnboundedSender<WsMessage>,
    ) -> Result<TunnelInfo, String> {
        // 分配端口
        let server_port = if let Some(port) = config.remote_port {
            if port >= self.port_start && port <= self.port_end && !self.is_port_used(port) {
                port
            } else {
                self.find_available_port().await?
            }
        } else {
            self.find_available_port().await?
        };

        // 绑定端口
        let listener = TcpListener::bind(format!("0.0.0.0:{}", server_port))
            .await
            .map_err(|e| format!("绑定端口 {} 失败: {}", server_port, e))?;

        let tunnel_id = Uuid::new_v4().to_string();
        let info = TunnelInfo {
            id: tunnel_id.clone(),
            client_id: client_id.to_string(),
            tunnel_type: config.tunnel_type,
            name: config
                .name
                .unwrap_or_else(|| format!("tunnel-{}", server_port)),
            local_addr: config.local_addr,
            local_port: config.local_port,
            server_port,
            state: "active".to_string(),
        };

        // 启动 accept 循环
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
        let mut shutdown_rx = shutdown_tx.subscribe();
        let connections = Arc::clone(&self.connections);
        let tid = tunnel_id.clone();
        let cid = client_id.to_string();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, addr)) => {
                                debug!("新连接 {} -> 隧道 {}", addr, tid);
                                let conn_id = Uuid::new_v4().to_string();
                                let (data_tx, mut data_rx) = mpsc::unbounded_channel::<Vec<u8>>();

                                connections.insert(conn_id.clone(), ConnectionState {
                                    tunnel_id: tid.clone(),
                                    client_id: cid.clone(),
                                    tx: data_tx,
                                });

                                // 通知客户端有新连接
                                let _ = client_tx.send(WsMessage::NewConnection {
                                    tunnel_id: tid.clone(),
                                    conn_id: conn_id.clone(),
                                });

                                let conns = Arc::clone(&connections);
                                let ctx = client_tx.clone();
                                let cid2 = conn_id.clone();

                                tokio::spawn(async move {
                                    let (mut read_half, mut write_half) = stream.into_split();
                                    let conn_id_r = cid2.clone();
                                    let ctx_r = ctx.clone();

                                    // 外部 -> 客户端
                                    let read_task = tokio::spawn(async move {
                                        let mut buf = [0u8; 8192];
                                        loop {
                                            match read_half.read(&mut buf).await {
                                                Ok(0) => break,
                                                Ok(n) => {
                                                    if ctx_r.send(WsMessage::Data {
                                                        conn_id: conn_id_r.clone(),
                                                        data: buf[..n].to_vec(),
                                                    }).is_err() {
                                                        break;
                                                    }
                                                }
                                                Err(_) => break,
                                            }
                                        }
                                    });

                                    // 客户端 -> 外部
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

                                    conns.remove(&cid2);
                                    let _ = ctx.send(WsMessage::CloseConnection { conn_id: cid2 });
                                });
                            }
                            Err(e) => {
                                error!("Accept 错误: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("隧道 {} 监听关闭", tid);
                        break;
                    }
                }
            }
        });

        self.tunnels.insert(
            tunnel_id.clone(),
            TunnelState {
                info: info.clone(),
                shutdown: Some(shutdown_tx),
            },
        );

        info!("隧道创建: {} -> 0.0.0.0:{}", tunnel_id, server_port);
        Ok(info)
    }

    async fn find_available_port(&self) -> Result<u16, String> {
        for port in self.port_start..=self.port_end {
            if !self.is_port_used(port) {
                if TcpListener::bind(format!("0.0.0.0:{}", port)).await.is_ok() {
                    return Ok(port);
                }
            }
        }
        Err("没有可用端口".to_string())
    }

    fn is_port_used(&self, port: u16) -> bool {
        self.tunnels
            .iter()
            .any(|t| t.value().info.server_port == port)
    }

    /// 关闭单个隧道
    pub fn close_tunnel(&self, tunnel_id: &str) -> Result<(), String> {
        // 从 tunnels 中移除
        let tunnel = self
            .tunnels
            .remove(tunnel_id)
            .map(|(_, t)| t)
            .ok_or_else(|| format!("隧道 {} 不存在", tunnel_id))?;

        // 发送 shutdown 信号关闭 TCP listener
        if let Some(shutdown) = tunnel.shutdown {
            let _ = shutdown.send(());
        }

        // 从所属客户端的 tunnel_ids 中移除
        if let Some(mut client) = self.clients.get_mut(&tunnel.info.client_id) {
            client.tunnel_ids.retain(|id| id != tunnel_id);
        }

        // 清理该隧道的所有连接
        let conn_ids: Vec<String> = self
            .connections
            .iter()
            .filter(|c| c.tunnel_id == tunnel_id)
            .map(|c| c.key().clone())
            .collect();
        for conn_id in conn_ids {
            self.connections.remove(&conn_id);
        }

        info!("隧道关闭: {}", tunnel_id);
        Ok(())
    }

    pub fn remove_client(&self, client_id: &str) {
        if let Some((_, client)) = self.clients.remove(client_id) {
            for tunnel_id in client.tunnel_ids {
                if let Some((_, tunnel)) = self.tunnels.remove(&tunnel_id) {
                    if let Some(shutdown) = tunnel.shutdown {
                        let _ = shutdown.send(());
                    }
                    info!("隧道移除: {}", tunnel_id);
                }
            }
            // 清理该客户端的所有连接
            let conn_ids: Vec<String> = self
                .connections
                .iter()
                .filter(|c| c.client_id == client_id)
                .map(|c| c.key().clone())
                .collect();
            for conn_id in conn_ids {
                self.connections.remove(&conn_id);
            }
            info!("客户端断开: {}", client_id);
        }
    }
}
