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

示例:
  # 暴露 SSH 服务
  cec-tunnel -s ws://server:8888/tunnel -n "office" -t tcp:22:10022

  # 暴露多个服务
  cec-tunnel -s wss://tunnel.example.com/tunnel \
             -n "dev-server" \
             -t tcp:22:10022 \
             -t tcp:3306:10306
"#)]
struct Args {
    /// 服务器地址
    #[arg(short, long, default_value = "ws://localhost:8888/tunnel")]
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
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("cec_tunnel={}", args.log_level).parse()?)
        )
        .init();

    if args.tunnel.is_empty() {
        eprintln!("错误: 至少需要指定一个隧道配置 (-t)");
        eprintln!("示例: cec-tunnel -s ws://server:8888/tunnel -t tcp:22:10022");
        std::process::exit(1);
    }

    info!("CEC Tunnel Client v{}", env!("CARGO_PKG_VERSION"));
    info!("服务器: {}", args.server);
    for t in &args.tunnel {
        info!("隧道: {}", t);
    }

    let client = tunnel::TunnelClient::new(&args.server, &args.name, &args.tunnel, args.token)?;
    client.run().await
}
