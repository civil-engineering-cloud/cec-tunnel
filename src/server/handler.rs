//! WebSocket 和 HTTP 处理器

use crate::common::protocol::{TunnelConfig, WsMessage};
use crate::manager::ServerState;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::{IntoResponse, Json},
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::debug;

/// GET /status — 服务状态概览
pub async fn get_status(State(state): State<ServerState>) -> impl IntoResponse {
    let clients = state.clients.len();
    let tunnels = state.tunnels.len();
    let connections = state.connections.len();
    Json(json!({
        "code": 0,
        "message": "success",
        "data": {
            "status": "running",
            "version": env!("CARGO_PKG_VERSION"),
            "clients": clients,
            "tunnels": tunnels,
            "connections": connections
        }
    }))
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: ServerState) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();
    let mut client_id: Option<String> = None;

    // 发送任务 — Data 用 Binary 帧，其他用 Text/JSON
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let ws_msg = match &msg {
                WsMessage::Data { conn_id, data } => {
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
            if ws_tx.send(ws_msg).await.is_err() {
                break;
            }
        }
    });

    // 接收处理
    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                    match ws_msg {
                        WsMessage::Register { client, tunnels } => {
                            match state.register_client(client, tunnels, tx.clone()).await {
                                Ok((id, tunnel_infos)) => {
                                    client_id = Some(id.clone());
                                    let _ = tx.send(WsMessage::RegisterResponse {
                                        success: true,
                                        client_id: id,
                                        tunnels: tunnel_infos,
                                        message: None,
                                    });
                                }
                                Err(e) => {
                                    let _ = tx.send(WsMessage::RegisterResponse {
                                        success: false,
                                        client_id: String::new(),
                                        tunnels: vec![],
                                        message: Some(e),
                                    });
                                }
                            }
                        }
                        WsMessage::Ping { timestamp } => {
                            let _ = tx.send(WsMessage::Pong { timestamp });
                        }
                        WsMessage::ConnectionReady { tunnel_id, conn_id } => {
                            debug!("连接就绪: {} / {}", tunnel_id, conn_id);
                        }
                        WsMessage::Data { conn_id, data } => {
                            if let Some(conn) = state.connections.get(&conn_id) {
                                let _ = conn.tx.send(data);
                            }
                        }
                        WsMessage::CloseConnection { conn_id } => {
                            state.connections.remove(&conn_id);
                        }
                        WsMessage::AddTunnelResponse { .. } => {
                            // 不再需要处理：隧道由 HTTP API 直接创建
                            debug!("忽略 AddTunnelResponse（隧道已由 HTTP API 创建）");
                        }
                        _ => {}
                    }
                }
            }
            Message::Binary(data) => {
                // Binary 帧: conn_id(36 bytes) + payload
                if data.len() > 36 {
                    let conn_id = String::from_utf8_lossy(&data[..36]).to_string();
                    let payload = data[36..].to_vec();
                    if let Some(conn) = state.connections.get(&conn_id) {
                        let _ = conn.tx.send(payload);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // 清理
    if let Some(id) = client_id {
        state.remove_client(&id);
    }
    send_task.abort();
}

pub async fn list_clients(State(state): State<ServerState>) -> impl IntoResponse {
    let clients: Vec<_> = state
        .clients
        .iter()
        .map(|c| {
            json!({
                "id": c.info.id,
                "name": c.info.name,
                "hostname": c.info.hostname,
                "os": c.info.os,
                "arch": c.info.arch,
                "version": c.info.version,
                "local_ip": c.info.local_ip,
                "tunnels": c.tunnel_ids.len()
            })
        })
        .collect();
    let total = clients.len();
    Json(json!({ "code": 0, "message": "success", "data": { "items": clients, "total": total } }))
}

pub async fn list_tunnels(State(state): State<ServerState>) -> impl IntoResponse {
    let tunnels: Vec<_> = state
        .tunnels
        .iter()
        .map(|t| {
            // 查找所属客户端名称
            let client_name = state
                .clients
                .get(&t.info.client_id)
                .map(|c| c.info.name.clone())
                .unwrap_or_default();
            // 从原子计数器读取实时流量
            let bytes_sent = t.bytes_sent.load(std::sync::atomic::Ordering::Relaxed);
            let bytes_recv = t.bytes_recv.load(std::sync::atomic::Ordering::Relaxed);
            json!({
                "id": t.info.id,
                "client_id": t.info.client_id,
                "client_name": client_name,
                "name": t.info.name,
                "tunnel_type": t.info.tunnel_type,
                "local_addr": t.info.local_addr,
                "local_port": t.info.local_port,
                "server_port": t.info.server_port,
                "state": t.info.state,
                "bytes_sent": bytes_sent,
                "bytes_recv": bytes_recv,
                "created_at": t.info.created_at,
                "last_active_at": t.info.last_active_at
            })
        })
        .collect();
    let total = tunnels.len();
    Json(json!({ "code": 0, "message": "success", "data": { "items": tunnels, "total": total } }))
}

pub async fn close_tunnel(
    State(state): State<ServerState>,
    Path(tunnel_id): Path<String>,
) -> impl IntoResponse {
    match state.close_tunnel(&tunnel_id) {
        Ok(_) => Json(json!({ "code": 0, "message": "success", "data": null })).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "code": 404, "message": e, "data": null })),
        )
            .into_response(),
    }
}

/// DELETE /api/clients/:id — 断开客户端连接
pub async fn disconnect_client(
    State(state): State<ServerState>,
    Path(client_id): Path<String>,
) -> impl IntoResponse {
    if state.clients.contains_key(&client_id) {
        state.remove_client(&client_id);
        Json(json!({ "code": 0, "message": "success", "data": null })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "code": 404, "message": "客户端不存在", "data": null })),
        )
            .into_response()
    }
}

/// 请求体：给客户端动态添加隧道
#[derive(Deserialize)]
pub struct AddTunnelRequest {
    /// 协议类型: tcp / udp
    pub tunnel_type: Option<String>,
    /// 客户端本地地址
    pub local_addr: Option<String>,
    /// 客户端本地端口
    pub local_port: u16,
    /// 服务端端口（可选，不传则自动分配）
    pub server_port: Option<u16>,
    /// 隧道名称
    pub name: Option<String>,
}

/// POST /api/clients/:id/tunnels — 给已连接的客户端动态添加隧道
pub async fn add_client_tunnel(
    State(state): State<ServerState>,
    Path(client_id): Path<String>,
    Json(body): Json<AddTunnelRequest>,
) -> impl IntoResponse {
    // 检查客户端是否在线
    let client_tx = match state.clients.get(&client_id) {
        Some(c) => c.tx.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "code": 404, "message": "客户端不在线", "data": null })),
            )
                .into_response();
        }
    };

    let tunnel_type = match body.tunnel_type.as_deref().unwrap_or("tcp") {
        "udp" => crate::common::protocol::TunnelType::Udp,
        _ => crate::common::protocol::TunnelType::Tcp,
    };

    let config = TunnelConfig {
        tunnel_type,
        local_addr: body.local_addr.unwrap_or_else(|| "127.0.0.1".to_string()),
        local_port: body.local_port,
        remote_port: body.server_port,
        name: body.name,
    };

    // 服务端先创建隧道（绑定端口），再通知客户端记录映射
    match state
        .add_tunnel_to_client(&client_id, config.clone(), client_tx.clone())
        .await
    {
        Ok(info) => {
            // 用 AddTunnel 通知客户端记录本地映射（不触发客户端回复）
            let _ = client_tx.send(WsMessage::AddTunnel {
                request_id: info.id.clone(),
                tunnel: TunnelConfig {
                    tunnel_type: info.tunnel_type.clone(),
                    local_addr: info.local_addr.clone(),
                    local_port: info.local_port,
                    remote_port: Some(info.server_port),
                    name: Some(info.name.clone()),
                },
            });
            Json(json!({ "code": 0, "message": "success", "data": info })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "code": 400, "message": e, "data": null })),
        )
            .into_response(),
    }
}
