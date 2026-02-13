# CEC-Tunnel - 内网穿透

轻量级内网穿透工具，包含客户端和服务端，类似 frp 但更简单。

## 技术栈

- 核心: Rust
- 协议: WebSocket (ws + wss 双端口)
- 平台: Linux / macOS / Windows

## 功能特性

- 🚀 反向隧道，无需公网 IP
- 🔒 双端口: 9998 (ws 明文) + 9999 (wss 加密)
- 🔄 自动重连
- 📦 单文件，无依赖
- 🖥️ 跨平台支持

## 安装

### 一键安装 (Linux / macOS)

```bash
# GitHub
curl -fsSL https://raw.githubusercontent.com/civil-engineering-cloud/cec-tunnel/main/install.sh | bash

# 国内加速 (Gitee)
curl -fsSL https://gitee.com/civil-engineering-cloud/cec-tunnel/raw/main/install.sh | MIRROR=gitee bash
```

### 手动下载

从 Releases 页面下载对应平台的二进制文件，每个文件都有独立下载链接：

- GitHub: https://github.com/civil-engineering-cloud/cec-tunnel/releases
- Gitee: https://gitee.com/civil-engineering-cloud/cec-tunnel/releases

#### 客户端 (cec-tunnel)

| 平台 | 架构 | 文件 |
|------|------|------|
| Linux | x86_64 | `cec-tunnel-linux-amd64` |
| Linux | ARM64 | `cec-tunnel-linux-arm64` |
| macOS | x86_64 | `cec-tunnel-darwin-amd64` |
| macOS | ARM64 (M1/M2) | `cec-tunnel-darwin-arm64` |
| Windows | x86_64 | `cec-tunnel-windows-amd64.exe` |

#### 服务端 (cec-tunnel-server)

| 平台 | 架构 | 文件 |
|------|------|------|
| Linux | x86_64 | `cec-tunnel-server-linux-amd64` |
| Linux | ARM64 | `cec-tunnel-server-linux-arm64` |
| macOS | x86_64 | `cec-tunnel-server-darwin-amd64` |
| macOS | ARM64 (M1/M2) | `cec-tunnel-server-darwin-arm64` |
| Windows | x86_64 | `cec-tunnel-server-windows-amd64.exe` |

## 快速开始

### 1. 部署服务端 (公网服务器)

```bash
# 下载
curl -fsSL https://github.com/civil-engineering-cloud/cec-tunnel/releases/latest/download/cec-tunnel-server-linux-amd64 -o cec-tunnel-server
chmod +x cec-tunnel-server

# 启动 (默认 9998 ws + 9999 wss)
./cec-tunnel-server

# 仅 ws 明文 (无证书时 wss 自动跳过)
./cec-tunnel-server --ws-port 9998

# 指定 TLS 证书启用 wss
./cec-tunnel-server --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem
```

### 2. 运行客户端 (内网机器)

```bash
# 一键安装
curl -fsSL https://raw.githubusercontent.com/civil-engineering-cloud/cec-tunnel/main/install.sh | bash

# 最简连接 (仅需指定服务端地址)
cec-tunnel -s ws://your-server:9998

# 指定名称和隧道 (-n 和 -t 可选)
cec-tunnel -s ws://your-server:9998 -n "office" -t tcp:22:10000

# 公网加密连接 (wss)
cec-tunnel -s wss://your-server:9999

# 后台运行
nohup cec-tunnel -s wss://your-server:9999 -n "my-client" &
```

### 3. 访问内网服务

```bash
ssh -p 10000 user@your-server
```

## 端口说明

| 端口 | 协议 | 用途 |
|------|------|------|
| 9998 | ws:// | 明文 WebSocket，内网/开发环境 |
| 9999 | wss:// | 加密 WebSocket，公网/生产环境 |
| 10000-20000 | TCP | 隧道映射端口范围 |

## 隧道配置格式

```
类型:本地端口:服务端端口
类型:本地地址:本地端口:服务端端口
```

### 示例

```bash
# 暴露 SSH (22 -> 10000)
cec-tunnel -s ws://server:9998 -t tcp:22:10000

# 暴露多个服务
cec-tunnel -s wss://server:9999 \
           -n "dev-server" \
           -t tcp:22:10000 \
           -t tcp:3306:10306 \
           -t tcp:6379:10379
```

## 架构

```
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│   外部用户      │         │   CEC Tunnel    │         │   内网机器      │
│                 │         │     Server      │         │                 │
│  ssh -p 10000   │────────▶│  ws:  9998      │◀────────│  cec-tunnel     │
│  your-server    │         │  wss: 9999      │         │  (内网)         │
└─────────────────┘         └─────────────────┘         └─────────────────┘
```

## TLS 证书配置

服务端默认从以下路径读取证书：
- 证书: `/etc/cec-tunnel/cert.pem`
- 私钥: `/etc/cec-tunnel/key.pem`

可通过参数自定义：
```bash
./cec-tunnel-server --tls-cert /path/to/cert.pem --tls-key /path/to/key.pem
```

如果证书文件不存在，wss 端口不会启动，仅提供 ws 明文服务。

## API 接口

```bash
# 通过 ws 端口访问
curl -m 5 http://server:9998/health
curl -m 5 http://server:9998/api/clients
curl -m 5 http://server:9998/api/tunnels

# 通过 wss 端口访问 (需 -k 跳过自签证书验证)
curl -m 5 -k https://server:9999/health
```

## 许可证

MIT License
