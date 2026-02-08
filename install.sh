#!/bin/bash
# CEC Tunnel 一键安装脚本
# 用法: curl -fsSL https://raw.githubusercontent.com/civil-engineering-cloud/cec-tunnel/main/install.sh | bash

set -e

REPO="civil-engineering-cloud/cec-tunnel"
INSTALL_DIR="/usr/local/bin"
BINARY="cec-tunnel"

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# 检测系统
detect_platform() {
  OS=$(uname -s | tr '[:upper:]' '[:lower:]')
  ARCH=$(uname -m)

  case "$OS" in
    linux)  PLATFORM="linux" ;;
    darwin) PLATFORM="darwin" ;;
    *)      error "不支持的操作系统: $OS (仅支持 Linux / macOS)" ;;
  esac

  case "$ARCH" in
    x86_64|amd64)   ARCH="amd64" ;;
    aarch64|arm64)   ARCH="arm64" ;;
    *)               error "不支持的架构: $ARCH (仅支持 x86_64 / arm64)" ;;
  esac

  FILENAME="${BINARY}-${PLATFORM}-${ARCH}"
  info "检测到系统: ${PLATFORM}/${ARCH}"
}

# 获取最新版本
get_latest_version() {
  info "获取最新版本..."
  VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

  if [ -z "$VERSION" ]; then
    # GitHub API 限流时尝试从 redirect 获取
    VERSION=$(curl -fsSI "https://github.com/${REPO}/releases/latest" 2>/dev/null \
      | grep -i '^location:' | sed 's|.*/tag/||' | tr -d '\r\n')
  fi

  if [ -z "$VERSION" ]; then
    error "无法获取最新版本，请检查网络或访问 https://github.com/${REPO}/releases"
  fi

  info "最新版本: ${VERSION}"
}

# 下载
download() {
  DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${FILENAME}"
  TMP_FILE=$(mktemp)

  info "下载 ${FILENAME}..."
  if command -v curl &>/dev/null; then
    curl -fSL --progress-bar -o "$TMP_FILE" "$DOWNLOAD_URL"
  elif command -v wget &>/dev/null; then
    wget -q --show-progress -O "$TMP_FILE" "$DOWNLOAD_URL"
  else
    error "需要 curl 或 wget"
  fi

  chmod +x "$TMP_FILE"
}

# 安装
install() {
  info "安装到 ${INSTALL_DIR}/${BINARY}..."

  if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP_FILE" "${INSTALL_DIR}/${BINARY}"
  else
    sudo mv "$TMP_FILE" "${INSTALL_DIR}/${BINARY}"
  fi

  # 验证
  if command -v "$BINARY" &>/dev/null; then
    info "安装成功! 版本: $(${BINARY} --version 2>/dev/null || echo ${VERSION})"
  else
    warn "已安装到 ${INSTALL_DIR}/${BINARY}，但不在 PATH 中"
    warn "请运行: export PATH=\"${INSTALL_DIR}:\$PATH\""
  fi
}

# 打印使用说明
print_usage() {
  echo ""
  echo -e "${GREEN}=== CEC Tunnel 安装完成 ===${NC}"
  echo ""
  echo "使用示例:"
  echo "  # 暴露 SSH 服务"
  echo "  cec-tunnel -s ws://your-server:8080/tunnel -n \"my-server\" -t tcp:22:10022"
  echo ""
  echo "  # 暴露多个服务"
  echo "  cec-tunnel -s ws://your-server:8080/tunnel -n \"office\" \\"
  echo "    -t tcp:22:10022 \\"
  echo "    -t tcp:3306:10306"
  echo ""
  echo "更多信息: https://github.com/${REPO}"
  echo ""
}

# 主流程
detect_platform
get_latest_version
download
install
print_usage
