# CEC Tunnel

轻量级内网穿透客户端，配合 CEC Gateway 服务端使用。

## 功能特性

- 🚀 反向隧道，无需公网 IP
- 🔒 WebSocket 安全连接
- 🔄 自动重连
- 📦 单文件，无依赖
- 🖥️ 支持 Linux、macOS、Windows

## 下载

从 [Releases](https://github.com/civil-engineering-cloud/cec-tunnel/releases) 下载对应平台的可执行文件：

| 平台 | 架构 | 文件 |
|------|------|------|
| Linux | x86_64 | cec-tunnel-linux-amd64 |
| Linux | ARM64 | cec-tunnel-linux-arm64 |
| macOS | x86_64 | cec-tunnel-darwin-amd64 |
| macOS | ARM64 (M1/M2) | cec-tunnel-darwin-arm64 |
| Windows | x86_64 | cec-tunnel-windows-amd64.exe |

## 快速开始

### Linux / macOS

```bash
# 下载
curl -LO https://github.com/civil-engineering-cloud/cec-tunnel/releases/latest/download/cec-tunnel-linux-amd64
chmod +x cec-tunnel-linux-amd64

# 运行
./cec-tunnel-linux-amd64 -s ws://gateway.example.com:8880/tunnel -n "my-server" -t tcp:22:10022
```

### Windows

```powershell
# 下载后直接运行
.\cec-tunnel-windows-amd64.exe -s ws://gateway.example.com:8880/tunnel -n "my-server" -t tcp:22:10022
```

## 使用方法

```bash
cec-tunnel [OPTIONS]

Options:
  -s, --server <URL>     Gateway 服务器地址 [默认: ws://localhost:8880/tunnel]
  -n, --name <NAME>      客户端名称 [默认: tunnel-client]
  -t, --tunnel <CONFIG>  隧道配置，可多次指定
      --token <TOKEN>    认证 Token
      --log-level <LVL>  日志级别 [默认: info]
  -h, --help             显示帮助
  -V, --version          显示版本
```

### 隧道配置格式

```
类型:本地端口:服务端端口
类型:本地地址:本地端口:服务端端口
```

### 示例

```bash
# 暴露 SSH 服务 (22 -> 10022)
cec-tunnel -s ws://gateway:8880/tunnel -n "office" -t tcp:22:10022

# 暴露多个服务
cec-tunnel -s wss://gateway.example.com/tunnel \
           -n "dev-server" \
           -t tcp:22:10022 \
           -t tcp:3306:10306 \
           -t tcp:6379:10379

# 暴露其他机器的服务
cec-tunnel -s ws://gateway:8880/tunnel -n "proxy" -t tcp:192.168.1.100:22:10022
```

## 作为系统服务运行

### Linux (systemd)

创建 `/etc/systemd/system/cec-tunnel.service`:

```ini
[Unit]
Description=CEC Tunnel Client
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/cec-tunnel -s ws://gateway:8880/tunnel -n "my-server" -t tcp:22:10022
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable cec-tunnel
sudo systemctl start cec-tunnel
```

### macOS (launchd)

创建 `~/Library/LaunchAgents/com.cec.tunnel.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.cec.tunnel</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/cec-tunnel</string>
        <string>-s</string>
        <string>ws://gateway:8880/tunnel</string>
        <string>-n</string>
        <string>my-server</string>
        <string>-t</string>
        <string>tcp:22:10022</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

```bash
launchctl load ~/Library/LaunchAgents/com.cec.tunnel.plist
```

## 从源码编译

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 编译
cargo build --release

# 可执行文件在 target/release/cec-tunnel
```

## License

MIT
