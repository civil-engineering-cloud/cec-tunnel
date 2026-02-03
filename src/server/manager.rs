//! 隧道管理器

use crate::common::protocol::{ClientInfo, TunnelConfig, TunnelInfo, TunnelType, WsMessage};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct ServerState {
    pub clients: Arc<DashMap<String, ClientState>>,
    pub tunnels: Arc<DashMap<String, TunnelState>>,
    pub connections: Arc<DashMap<String, ConnectionState>>,
    pub port_start: u16,
    pub port_end: u16,
    pub auth_token: Option<String>,
}

pub struct ClientState {
    pub info: ClientInfo,
    pub tx: mpsc::UnboundedSender<WsMessage>,
    pub tunnel_ids: Vec<String>,
}

pub struct TunnelState {
    pub info: TunnelInfo,
    pub listener: Option<TcpListener>,
}

pub struct ConnectionState {
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
        let client_id = client.id.clone();
        let mut tunnel_infos = Vec::new();
        let mut tunnel_ids = Vec::new();

        for config in tunnels {
            match self.create_tunnel(&client_id, config).await {
                Ok(info) => {
                    tunnel_ids.push(info.id.clone());
                    tunnel_infos.push(info);
                }
                Err(e) => {
                    warn!("创建隧道失败: {}", e);
                }
            }
        }

        self.clients.insert(
            client_id.clone(),
            ClientState {
                info: client,
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
    ) -> Result<TunnelInfo, String> {
        let server_port = if let Some(port) = config.remote_port {
            if port >= self.port_start && port <= self.port_end {
                port
            } else {
                self.find_available_port().await?
            }
        } else {
            self.find_available_port().await?
        };

        let listener = TcpListener::bind(format!("0.0.0.0:{}", server_port))
            .await
            .map_err(|e| format!("绑定端口 {} 失败: {}", server_port, e))?;

        let tunnel_id = Uuid::new_v4().to_string();
        let info = TunnelInfo {
            id: tunnel_id.clone(),
            client_id: client_id.to_string(),
            tunnel_type: config.tunnel_type,
            name: config.name.unwrap_or_else(|| format!("tunnel-{}", server_port)),
            local_addr: config.local_addr,
            local_port: config.local_port,
            server_port,
            state: "active".to_string(),
        };

        self.tunnels.insert(
            tunnel_id.clone(),
            TunnelState {
                info: info.clone(),
                listener: Some(listener),
            },
        );

        info!(
            "隧道创建: {} -> 0.0.0.0:{}",
            tunnel_id, server_port
        );

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

    pub fn remove_client(&self, client_id: &str) {
        if let Some((_, client)) = self.clients.remove(client_id) {
            for tunnel_id in client.tunnel_ids {
                self.tunnels.remove(&tunnel_id);
                info!("隧道移除: {}", tunnel_id);
            }
            info!("客户端断开: {}", client_id);
        }
    }

    pub fn send_to_client(&self, client_id: &str, msg: WsMessage) {
        if let Some(client) = self.clients.get(client_id) {
            if let Err(e) = client.tx.send(msg) {
                error!("发送消息失败: {}", e);
            }
        }
    }
}
