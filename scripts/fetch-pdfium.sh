#!/usr/bin/env bash
# 下载 PDFium 动态库（三端）。
# 用法: ./scripts/fetch-pdfium.sh [macos-arm64|macos-x64|linux|windows]
# 默认: 当前平台

set -e

PLATFORM=${1:-$(uname -s)-$(uname -m)}
NATIVE_DIR="$(dirname "$0")/../native/pdfium"

case "$PLATFORM" in
  Darwin-arm64|darwin-arm64|macos-arm64) ASSET="pdfium-mac-arm64.tgz" ;;
  Darwin-x86_64|darwin-x64|macos-x64)    ASSET="pdfium-mac-x64.tgz" ;;
  Linux-x86_64|linux-x64|linux)          ASSET="pdfium-linux-x64.tgz" ;;
  Linux-aarch64|linux-arm64)             ASSET="pdfium-linux-arm64.tgz" ;;
  MINGW*|Windows*|windows)               ASSET="pdfium-win-x64.tgz" ;;
  *) echo "不支持的平台: $PLATFORM"; exit 1 ;;
esac

URL="https://github.com/bblanchon/pdfium-binaries/releases/latest/download/$ASSET"
TARGET_DIR="$NATIVE_DIR"

echo "下载 PDFium: $ASSET"
echo "  → $URL"
mkdir -p "$TARGET_DIR"
curl -sL "$URL" | tar -xzf - -C "$TARGET_DIR"
echo "完成: $TARGET_DIR"

# 显示库文件
echo ""
echo "库文件:"
find "$TARGET_DIR" -name "*.dylib" -o -name "*.so" -o -name "*.dll" | head -5

# 设置环境变量提示
case "$PLATFORM" in
  Darwin-*|darwin-*|macos-*)
    echo ""
    echo "开发时设置: export DYLD_LIBRARY_PATH=\"$TARGET_DIR/lib:\$DYLD_LIBRARY_PATH\"" ;;
  Linux-*|linux-*)
    echo ""
    echo "开发时设置: export LD_LIBRARY_PATH=\"$TARGET_DIR/lib:\$LD_LIBRARY_PATH\"" ;;
esac
