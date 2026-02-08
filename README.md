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

# 启动 (默认端口 8888)
./cec-tunnel-server
```

### 2. 运行客户端 (内网机器)

```bash
# 一键安装
curl -fsSL https://raw.githubusercontent.com/civil-engineering-cloud/cec-tunnel/main/install.sh | bash

# 连接并暴露 SSH
cec-tunnel -s ws://your-server:8888/tunnel -n "office" -t tcp:22:10000
```

### 3. 访问内网服务

```bash
ssh -p 10000 user@your-server
```

## 隧道配置格式

```
类型:本地端口:服务端端口
类型:本地地址:本地端口:服务端端口
```

### 示例

```bash
# 暴露 SSH (22 -> 10000)
cec-tunnel -s ws://server:8888/tunnel -t tcp:22:10000

# 暴露多个服务
cec-tunnel -s ws://server:8888/tunnel \
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
│  ssh -p 10000   │────────▶│  (公网:8888)    │◀────────│  cec-tunnel     │
│  your-server    │         │                 │         │  (内网)         │
└─────────────────┘         │   WebSocket     │─────────│                 │
                            └─────────────────┘         └─────────────────┘
```

## API 接口

```bash
curl -m 5 http://server:8888/health          # 健康检查
curl -m 5 http://server:8888/api/clients      # 已连接客户端
curl -m 5 http://server:8888/api/tunnels      # 所有隧道
```

## 许可证

MIT License
