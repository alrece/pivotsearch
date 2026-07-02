# pivotsearch Makefile — 统一命令入口

.PHONY: help check build test clippy run clean cleanroom dev build-desktop

help: ## 显示所有命令
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'

check: ## 全 workspace 编译检查
	cargo check

build: ## 发布构建
	cargo build --release

test: ## 全 workspace 测试
	cargo test

clippy: ## Lint（警告即错误）
	cargo clippy --all-targets -- -D warnings

run: ## 运行 CLI（开发期）
	cargo run --bin pivotsearch

cleanroom: ## 净室合规检查（验证无 DocFetcher 代码残留）
	@HITS=$$(grep -ri "docfetcher\|net.sourceforge.docfetcher" crates/ src/ src-tauri/ 2>/dev/null | grep -v "target/" || true); \
	if [ -z "$$HITS" ]; then echo "✅ PASS: 净室合规，无 DocFetcher 标识符残留"; else echo "❌ FAIL:"; echo "$$HITS"; exit 1; fi

deps-check: ## 依赖方向验证（core 不应 import 具体实现）
	@CORE_DEPS=$$(grep -E "pivotsearch-(parser|index|watcher|queue|search|ocr)" crates/core/src/*.rs crates/core/Cargo.toml 2>/dev/null || true); \
	if [ -z "$$CORE_DEPS" ]; then echo "✅ PASS: core 只依赖 contracts"; else echo "❌ FAIL: core 违反依赖方向铁律"; echo "$$CORE_DEPS"; exit 1; fi

dev: ## Tauri 桌面端开发模式（Phase 4 起）
	cargo tauri dev

build-desktop: ## Tauri 桌面端打包（Phase 5 起）
	cargo tauri build

clean: ## 清理构建产物
	cargo clean
	rm -rf node_modules dist src-tauri/target src-tauri/gen
