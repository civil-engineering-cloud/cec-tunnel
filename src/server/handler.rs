//! WebSocket 和 HTTP 处理器

use crate::common::protocol::WsMessage;
use crate::manager::ServerState;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::{IntoResponse, Json},
};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::info;

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

    // 发送任务
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_tx.send(Message::Text(json)).await.is_err() {
                    break;
                }
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
                            info!("连接就绪: {} / {}", tunnel_id, conn_id);
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
    Json(json!({ "clients": clients }))
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
    Json(json!({ "tunnels": tunnels }))
}
