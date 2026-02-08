# CEC-Tunnel - 内网穿透

轻量级内网穿透工具，包含客户端和服务端，类似 frp 但更简单。

## 技术栈

- 核心: Rust
- 协议: WebSocket (支持 wss)
- 平台: Linux / macOS / Windows

## 功能特性

- 🚀 反向隧道，无需公网 IP
- 🔒 WebSocket 安全连接 (支持 wss)
- 🔄 自动重连
- 📦 单文件，无依赖
- 🖥️ 跨平台支持

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

### Docker 部署

```bash
docker compose -f docker-compose.dev.yml up -d
```

### 访问地址

- 统一入口: http://localhost:8370

### 1. 部署服务端 (公网服务器)

```bash
./cec-tunnel-server-linux-amd64 -p 8370
```

### 2. 运行客户端 (内网机器)

```bash
./cec-tunnel-linux-amd64 -s ws://your-server:8370/tunnel -n "office" -t tcp:22:10022
```

### 3. 访问内网服务

```bash
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
cec-tunnel -s ws://server:8370/tunnel -t tcp:22:10022

# 暴露多个服务
cec-tunnel -s ws://server:8370/tunnel \
           -n "dev-server" \
           -t tcp:22:10022 \
           -t tcp:3306:10306 \
           -t tcp:6379:10379
```

## 架构

```
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│   外部用户      │         │   CEC Tunnel    │         │   内网机器      │
│                 │         │     Server      │         │                 │
│  ssh -p 10022   │────────▶│  (公网:8370)    │◀────────│  cec-tunnel     │
│  your-server    │         │                 │         │  (内网)         │
└─────────────────┘         │   WebSocket     │─────────│                 │
                            └─────────────────┘         └─────────────────┘
```

## API 接口

```bash
curl -m 5 http://server:8370/health          # 健康检查
curl -m 5 http://server:8370/api/clients      # 已连接客户端
curl -m 5 http://server:8370/api/tunnels      # 所有隧道
```

## 许可证

MIT License
