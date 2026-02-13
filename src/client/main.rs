//! CEC Tunnel Client
//!
//! 内网穿透客户端，连接到服务端建立反向隧道。

mod tunnel;

#[path = "../common/mod.rs"]
mod common;

use anyhow::Result;
use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "cec-tunnel")]
#[command(version, about = "CEC Tunnel Client - 内网穿透客户端")]
#[command(long_about = r#"
CEC Tunnel 是一个轻量级的内网穿透客户端。

它通过 WebSocket 连接到服务端，建立反向隧道，
让外部用户可以通过服务端端口访问内网服务。

服务端端口:
  9998 — ws://  (明文，内网/开发)
  9999 — wss:// (加密，公网/生产)

示例:
  # 最简连接
  cec-tunnel -s ws://server:9998

  # 指定名称和隧道 (-n 和 -t 可选)
  cec-tunnel -s wss://server:9999 -n "office" -t tcp:22:10022

  # 暴露多个服务
  cec-tunnel -s wss://tunnel.example.com:9999 \
             -n "dev-server" \
             -t tcp:22:10022 \
             -t tcp:3306:10306
"#)]
struct Args {
    /// 服务器地址 (ws://host:9998 明文, wss://host:9999 加密)
    #[arg(short, long, default_value = "ws://localhost:9998")]
    server: String,

    /// 客户端名称
    #[arg(short, long, default_value = "tunnel-client")]
    name: String,

    /// 隧道配置: type:local_port:remote_port
    #[arg(short, long)]
    tunnel: Vec<String>,

    /// 认证 Token
    #[arg(long)]
    token: Option<String>,

    /// 日志级别
    #[arg(long, default_value = "info")]
    log_level: String,
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
        )
        .init();

    info!("CEC Tunnel Client v{}", env!("CARGO_PKG_VERSION"));

    // 自动拼接 /tunnel 路径，用户无需手动添加
    let server_url = if args.server.ends_with("/tunnel") {
        args.server.clone()
    } else {
        let base = args.server.trim_end_matches('/');
        format!("{}/tunnel", base)
    };

    info!("服务器: {}", server_url);

    if args.tunnel.is_empty() {
        info!("未指定隧道，仅建立连接，等待服务端分配...");
    } else {
        for t in &args.tunnel {
            info!("隧道: {}", t);
        }
    }

    let client = tunnel::TunnelClient::new(&server_url, &args.name, &args.tunnel, args.token)?;
    client.run().await
}
