//! CEC Tunnel Client
//!
//! 内网穿透客户端，连接到公网 Gateway 服务端，
//! 建立反向隧道，让外部用户可以访问内网服务。
//!
//! ## 使用方法
//! ```bash
//! cec-tunnel -s wss://gateway.example.com/tunnel \
//!            -n "my-server" \
//!            -t tcp:22:10022 \
//!            -t tcp:3306:10306
//! ```

mod client;
mod protocol;

use anyhow::Result;
use clap::Parser;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "cec-tunnel")]
#[command(version, about = "CEC Gateway Tunnel Client - 内网穿透客户端")]
#[command(long_about = r#"
CEC Tunnel 是一个轻量级的内网穿透客户端。

它通过 WebSocket 连接到 Gateway 服务端，建立反向隧道，
让外部用户可以通过服务端端口访问内网服务。

示例:
  # 暴露 SSH 服务
  cec-tunnel -s ws://gateway:8880/tunnel -n "office" -t tcp:22:10022

  # 暴露多个服务
  cec-tunnel -s wss://gateway.example.com/tunnel \
             -n "dev-server" \
             -t tcp:22:10022 \
             -t tcp:3306:10306 \
             -t tcp:6379:10379
"#)]
struct Args {
    /// Gateway 服务器地址
    #[arg(short, long, default_value = "ws://localhost:8880/tunnel")]
    server: String,

    /// 客户端名称
    #[arg(short, long, default_value = "tunnel-client")]
    name: String,

    /// 隧道配置，格式: type:local_port:remote_port 或 type:local_addr:local_port:remote_port
    /// 例如: tcp:22:10022 或 tcp:192.168.1.100:22:10022
    #[arg(short, long)]
    tunnel: Vec<String>,

    /// 认证 Token
    #[arg(long)]
    token: Option<String>,

    /// 日志级别 (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("cec_tunnel={}", args.log_level).parse()?)
        )
        .init();

    if args.tunnel.is_empty() {
        eprintln!("错误: 至少需要指定一个隧道配置 (-t)");
        eprintln!("示例: cec-tunnel -s ws://gateway:8880/tunnel -t tcp:22:10022");
        std::process::exit(1);
    }

    info!("CEC Tunnel Client v{}", env!("CARGO_PKG_VERSION"));
    info!("服务器: {}", args.server);
    for t in &args.tunnel {
        info!("隧道: {}", t);
    }

    let client = client::TunnelClient::new(&args.server, &args.name, &args.tunnel, args.token)?;
    client.run().await
}
