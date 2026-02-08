//! WebSocket 和 HTTP 处理器

use crate::common::protocol::WsMessage;
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
use serde_json::json;
use tokio::sync::mpsc;
use tracing::debug;

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
            json!({
                "id": t.info.id,
                "client_id": t.info.client_id,
                "name": t.info.name,
                "local": format!("{}:{}", t.info.local_addr, t.info.local_port),
                "server_port": t.info.server_port,
                "state": t.info.state
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
