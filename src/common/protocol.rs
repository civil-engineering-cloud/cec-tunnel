//! WebSocket 协议消息定义

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelType {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub hostname: String,
    pub local_ip: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub tunnel_type: TunnelType,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_port: Option<u16>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInfo {
    pub id: String,
    pub client_id: String,
    pub tunnel_type: TunnelType,
    pub name: String,
    pub local_addr: String,
    pub local_port: u16,
    pub server_port: u16,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    Register {
        client: ClientInfo,
        tunnels: Vec<TunnelConfig>,
    },
    RegisterResponse {
        success: bool,
        client_id: String,
        tunnels: Vec<TunnelInfo>,
        message: Option<String>,
    },
    NewConnection {
        tunnel_id: String,
        conn_id: String,
    },
    ConnectionReady {
        tunnel_id: String,
        conn_id: String,
    },
    Data {
        conn_id: String,
        #[serde(with = "base64_bytes")]
        data: Vec<u8>,
    },
    CloseConnection {
        conn_id: String,
    },
    Ping {
        timestamp: i64,
    },
    Pong {
        timestamp: i64,
    },
    Error {
        code: i32,
        message: String,
    },
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

impl TunnelConfig {
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();

        let (tunnel_type, local_addr, local_port, remote_port) = match parts.len() {
            // type:local_port:remote_port
            3 => {
                let t = match parts[0] {
                    "tcp" => TunnelType::Tcp,
                    "udp" => TunnelType::Udp,
                    _ => return None,
                };
                let lp: u16 = parts[1].parse().ok()?;
                let rp: u16 = parts[2].parse().ok()?;
                (t, "127.0.0.1".to_string(), lp, Some(rp))
            }
            // type:local_addr:local_port:remote_port
            4 => {
                let t = match parts[0] {
                    "tcp" => TunnelType::Tcp,
                    "udp" => TunnelType::Udp,
                    _ => return None,
                };
                let la = parts[1].to_string();
                let lp: u16 = parts[2].parse().ok()?;
                let rp: u16 = parts[3].parse().ok()?;
                (t, la, lp, Some(rp))
            }
            _ => return None,
        };

        Some(Self {
            tunnel_type,
            local_addr,
            local_port,
            remote_port,
            name: None,
        })
    }
}
