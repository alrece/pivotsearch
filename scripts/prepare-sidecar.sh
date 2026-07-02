#!/usr/bin/env bash
# 打包前编译 psearch CLI 并复制到 Tauri sidecar 期望位置。
# Tauri 要求 sidecar 文件名带 target-triple 后缀。
#
# 由 tauri.conf.json 的 beforeBundleCommand 自动调用。

set -euo pipefail

cd "$(dirname "$0")/.."

# 获取当前编译目标 triple
TARGET="${TARGET_TRIPLE:-$(rustc -vV | sed -n 's/host: //p')}"
echo "📦 编译 psearch CLI (target: $TARGET)..."

# 编译 psearch（release 模式，与 Tauri 主程序一致）
cargo build --release -p psearch

# 复制到 sidecar 期望位置
BIN_DIR="src-tauri/binaries"
mkdir -p "$BIN_DIR"

EXT=""
[[ "$TARGET" == *windows* ]] && EXT=".exe"

cp "target/release/psearch" "$BIN_DIR/psearch-${TARGET}${EXT}"
chmod +x "$BIN_DIR/psearch-${TARGET}${EXT}"

echo "✅ sidecar 就位: $BIN_DIR/psearch-${TARGET}${EXT}"
