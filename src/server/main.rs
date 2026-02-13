//! CEC Tunnel Server
//!
//! 内网穿透服务端，同时监听两个端口：
//! - 9998: ws:// (明文 WebSocket)
//! - 9999: wss:// (TLS 加密 WebSocket)

mod handler;
mod manager;

#[path = "../common/mod.rs"]
mod common;

use anyhow::Result;
use axum::{
    routing::{delete, get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(name = "cec-tunnel-server")]
#[command(version, about = "CEC Tunnel Server - 内网穿透服务端")]
struct Args {
    /// 监听地址
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// ws:// 明文端口
    #[arg(long, default_value = "9998")]
    ws_port: u16,

    /// wss:// 加密端口
    #[arg(long, default_value = "9999")]
    wss_port: u16,

    /// 兼容旧版 -p 参数（映射到 ws_port）
    #[arg(short, long)]
    port: Option<u16>,

    /// 隧道端口范围起始
    #[arg(long, default_value = "10000")]
    port_start: u16,

    /// 隧道端口范围结束
    #[arg(long, default_value = "20000")]
    port_end: u16,

    /// 认证 Token (可选)
    #[arg(long)]
    token: Option<String>,

    /// TLS 证书文件路径 (PEM 格式)
    #[arg(long, default_value = "/etc/cec-tunnel/cert.pem")]
    tls_cert: String,

    /// TLS 私钥文件路径 (PEM 格式)
    #[arg(long, default_value = "/etc/cec-tunnel/key.pem")]
    tls_key: String,

    /// 启用 ws:// 明文端口
    #[arg(long)]
    enable_ws: bool,

    /// 启用 wss:// 加密端口
    #[arg(long)]
    enable_wss: bool,

    /// 日志级别
    #[arg(long, default_value = "info")]
    log_level: String,
}

fn build_router(state: manager::ServerState) -> Router {
    Router::new()
        .route("/", get(|| async { "CEC Tunnel Server" }))
        .route("/health", get(|| async { "OK" }))
        .route("/status", get(handler::get_status))
        .route("/tunnel", get(handler::ws_handler))
        .route("/api/clients", get(handler::list_clients))
        .route("/api/clients/:id", delete(handler::disconnect_client))
        .route("/api/tunnels", get(handler::list_tunnels))
        .route("/api/tunnels/:id", delete(handler::close_tunnel))
        .route("/api/clients/:id/tunnels", post(handler::add_client_tunnel))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<()> {
    // rustls 0.23+ 需要显式安装 CryptoProvider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("cec_tunnel={}", args.log_level).parse()?)
                .add_directive("tower_http=debug".parse()?),
        )
        .init();

    info!("CEC Tunnel Server v{}", env!("CARGO_PKG_VERSION"));

    let ws_port = args.port.unwrap_or(args.ws_port);
    info!("端口范围: {} - {}", args.port_start, args.port_end);

    let state = manager::ServerState::new(args.port_start, args.port_end, args.token);
    let app = build_router(state);

    if !args.enable_ws && !args.enable_wss {
        eprintln!("错误: ws 和 wss 都未启用，至少需要启用一个");
        std::process::exit(1);
    }

    // 启动 ws:// 明文服务
    let ws_handle = if args.enable_ws {
        let ws_addr: SocketAddr = format!("{}:{}", args.bind, ws_port).parse()?;
        let ws_app = app.clone();
        info!("ws://  -> {}", ws_addr);
        Some(tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(ws_addr).await.unwrap();
            axum::serve(listener, ws_app).await.unwrap();
        }))
    } else {
        info!("ws:// 端口 {} 未启用 (--enable-ws 未设置)", ws_port);
        None
    };

    // 启动 wss:// 加密服务
    let wss_handle = if args.enable_wss {
        let wss_addr: SocketAddr = format!("{}:{}", args.bind, args.wss_port).parse()?;
        let has_tls = tokio::fs::metadata(&args.tls_cert).await.is_ok()
            && tokio::fs::metadata(&args.tls_key).await.is_ok();

        if has_tls {
            let config = RustlsConfig::from_pem_file(&args.tls_cert, &args.tls_key).await?;
            info!("wss:// -> {} (TLS: {}, {})", wss_addr, args.tls_cert, args.tls_key);
            let wss_app = app;
            Some(tokio::spawn(async move {
                axum_server::bind_rustls(wss_addr, config)
                    .serve(wss_app.into_make_service())
                    .await
                    .unwrap();
            }))
        } else {
            warn!(
                "TLS 证书未找到 ({}, {})，wss:// 端口 {} 无法启动",
                args.tls_cert, args.tls_key, args.wss_port
            );
            if ws_handle.is_none() {
                eprintln!("错误: wss 需要 TLS 证书但未找到，且 ws 也未启用");
                std::process::exit(1);
            }
            warn!("回退到仅 ws:// 明文服务");
            None
        }
    } else {
        info!("wss:// 端口 {} 未启用 (--enable-wss 未设置)", args.wss_port);
        None
    };

    match (ws_handle, wss_handle) {
        (Some(ws), Some(wss)) => {
            tokio::select! {
                r = ws => { r?; }
                r = wss => { r?; }
            }
        }
        (Some(ws), None) => { ws.await?; }
        (None, Some(wss)) => { wss.await?; }
        (None, None) => unreachable!(),
    }

    Ok(())
}
