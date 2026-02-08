//! CEC Tunnel Server
//!
//! 内网穿透服务端，接收客户端连接，管理隧道和端口映射。

mod handler;
mod manager;

#[path = "../common/mod.rs"]
mod common;

use anyhow::Result;
use axum::{routing::{get, delete}, Router};
use clap::Parser;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "cec-tunnel-server")]
#[command(version, about = "CEC Tunnel Server - 内网穿透服务端")]
struct Args {
    /// 监听地址
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// WebSocket 端口
    #[arg(short, long, default_value = "8888")]
    port: u16,

    /// 隧道端口范围起始
    #[arg(long, default_value = "10000")]
    port_start: u16,

    /// 隧道端口范围结束
    #[arg(long, default_value = "20000")]
    port_end: u16,

    /// 认证 Token (可选)
    #[arg(long)]
    token: Option<String>,

    /// 日志级别
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("cec_tunnel={}", args.log_level).parse()?)
                .add_directive("tower_http=debug".parse()?),
        )
        .init();

    info!("CEC Tunnel Server v{}", env!("CARGO_PKG_VERSION"));
    info!("WebSocket: ws://{}:{}/tunnel", args.bind, args.port);
    info!("端口范围: {} - {}", args.port_start, args.port_end);

    let state = manager::ServerState::new(args.port_start, args.port_end, args.token);

    let app = Router::new()
        .route("/", get(|| async { "CEC Tunnel Server" }))
        .route("/health", get(|| async { "OK" }))
        .route("/tunnel", get(handler::ws_handler))
        .route("/api/clients", get(handler::list_clients))
        .route("/api/tunnels", get(handler::list_tunnels))
        .route("/api/tunnels/:id", delete(handler::close_tunnel))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    info!("服务启动: {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
