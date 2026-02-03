# CEC Tunnel

轻量级内网穿透工具，包含客户端和服务端，类似 frp 但更简单。

## 功能特性

- 🚀 反向隧道，无需公网 IP
- 🔒 WebSocket 安全连接 (支持 wss)
- 🔄 自动重连
- 📦 单文件，无依赖
- 🖥️ 支持 Linux、macOS、Windows
- 🎯 客户端 + 服务端完整方案

## 下载

从 [Releases](https://github.com/civil-engineering-cloud/cec-tunnel/releases) 下载：

| 组件 | 平台 | 文件 |
|------|------|------|
| 客户端 | Linux x64 | cec-tunnel-linux-amd64 |
| 客户端 | Linux ARM64 | cec-tunnel-linux-arm64 |
| 客户端 | macOS x64 | cec-tunnel-darwin-amd64 |
| 客户端 | macOS ARM64 | cec-tunnel-darwin-arm64 |
| 客户端 | Windows | cec-tunnel-windows-amd64.exe |
| 服务端 | Linux x64 | cec-tunnel-server-linux-amd64 |
| 服务端 | Linux ARM64 | cec-tunnel-server-linux-arm64 |
| 服务端 | macOS x64 | cec-tunnel-server-darwin-amd64 |
| 服务端 | macOS ARM64 | cec-tunnel-server-darwin-arm64 |
| 服务端 | Windows | cec-tunnel-server-windows-amd64.exe |

## 快速开始

### 1. 部署服务端 (公网服务器)

```bash
# 下载
curl -LO https://github.com/civil-engineering-cloud/cec-tunnel/releases/latest/download/cec-tunnel-server-linux-amd64
chmod +x cec-tunnel-server-linux-amd64

# 运行
./cec-tunnel-server-linux-amd64 -p 8880
```

服务端参数：
```
-b, --bind <ADDR>       监听地址 [默认: 0.0.0.0]
-p, --port <PORT>       WebSocket 端口 [默认: 8880]
    --port-start <PORT> 隧道端口范围起始 [默认: 10000]
    --port-end <PORT>   隧道端口范围结束 [默认: 20000]
    --token <TOKEN>     认证 Token (可选)
```

### 2. 运行客户端 (内网机器)

```bash
# 下载
curl -LO https://github.com/civil-engineering-cloud/cec-tunnel/releases/latest/download/cec-tunnel-linux-amd64
chmod +x cec-tunnel-linux-amd64

# 暴露 SSH 服务
./cec-tunnel-linux-amd64 -s ws://your-server:8880/tunnel -n "office" -t tcp:22:10022
```

客户端参数：
```
-s, --server <URL>     服务器地址 [默认: ws://localhost:8880/tunnel]
-n, --name <NAME>      客户端名称 [默认: tunnel-client]
-t, --tunnel <CONFIG>  隧道配置，可多次指定
    --token <TOKEN>    认证 Token
```

### 3. 访问内网服务

```bash
# 通过服务端访问内网 SSH
ssh -p 10022 user@your-server
```

## 隧道配置格式

```
类型:本地端口:服务端端口
类型:本地地址:本地端口:服务端端口
```

### 示例

```bash
# 暴露 SSH (22 -> 10022)
cec-tunnel -s ws://server:8880/tunnel -t tcp:22:10022

# 暴露多个服务
cec-tunnel -s ws://server:8880/tunnel \
           -n "dev-server" \
           -t tcp:22:10022 \
           -t tcp:3306:10306 \
           -t tcp:6379:10379

# 暴露其他机器的服务
cec-tunnel -s ws://server:8880/tunnel -t tcp:192.168.1.100:22:10022
```

## 作为系统服务运行

### 服务端 (systemd)

```ini
# /etc/systemd/system/cec-tunnel-server.service
[Unit]
Description=CEC Tunnel Server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/cec-tunnel-server -p 8880
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

### 客户端 (systemd)

```ini
# /etc/systemd/system/cec-tunnel.service
[Unit]
Description=CEC Tunnel Client
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/cec-tunnel -s ws://server:8880/tunnel -n "my-server" -t tcp:22:10022
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable cec-tunnel-server  # 或 cec-tunnel
sudo systemctl start cec-tunnel-server
```

## API 接口

服务端提供 HTTP API：

```bash
# 健康检查
curl http://server:8880/health

# 查看已连接客户端
curl http://server:8880/api/clients

# 查看所有隧道
curl http://server:8880/api/tunnels
```

## 从源码编译

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 编译
cargo build --release

# 可执行文件
# target/release/cec-tunnel
# target/release/cec-tunnel-server
```

## 架构

```
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│   外部用户      │         │   CEC Tunnel    │         │   内网机器      │
│                 │         │     Server      │         │                 │
│  ssh -p 10022   │────────▶│   (公网:8880)   │◀────────│  cec-tunnel     │
│  your-server    │         │                 │         │  (内网)         │
└─────────────────┘         │   端口 10022    │         │                 │
                            │       ↓         │         │   SSH :22       │
                            │   WebSocket     │─────────│                 │
                            └─────────────────┘         └─────────────────┘
```

## License

MIT
