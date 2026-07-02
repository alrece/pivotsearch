# 贡献指南

感谢你对 pivotsearch 的兴趣！欢迎提交 Issue、Pull Request 或建议。

## 开发环境

```bash
# 前置依赖
# - Rust 1.75+ (推荐 rustup 安装)
# - Node.js 20+ 和 pnpm
# - macOS: Xcode Command Line Tools
# - Linux: libwebkit2gtk-4.1-dev librsvg2-dev patchelf
# - Windows: MSVC Build Tools

git clone https://github.com/alrece/pivotsearch.git
cd pivotsearch
pnpm install

# 开发
pnpm tauri dev          # 桌面端热重载
cargo test --workspace  # 后端测试
pnpm build              # 前端构建
```

## 项目约定

### 依赖方向铁律
- `core` 编排层**只依赖** `contracts` trait，绝不 import 具体实现
- `contracts` 是依赖终点（不依赖任何内部 crate）
- 只有 `cli` / `src-tauri`（组装根）能 import 具体实现

### 净室红线
本项目借鉴经典桌面搜索工具的**设计逻辑**（mtime 增量、Parser 注册表等公共模式），但**不复制任何源代码/类名/标识符**。提交前运行：

```bash
make cleanroom    # 检查零残留
```

### 提交前检查
```bash
cargo check --workspace                    # 编译
cargo test --workspace                     # 测试
make cleanroom                             # 净室
make deps-check                            # 依赖方向
pnpm build                                 # 前端
```

### Tantivy 关键约束
- schema 启动时定死（变更需 reindex）
- 同一索引目录同时只能一个 writer（单工作线程）
- 无原生 upsert（delete_term + add_document）

## 提交 Pull Request

1. Fork 仓库并创建分支：`git checkout -b feature/my-feature`
2. 确保通过所有检查（编译 + 测试 + 净室）
3. 提交清晰的 commit message（中文或英文均可）
4. 创建 PR，描述改动内容和动机

## 提交 Issue

- Bug 报告：附上复现步骤、系统环境、错误信息
- 功能建议：描述使用场景和期望行为

## 许可证

提交的代码将在 [Apache License 2.0](LICENSE) 下发布。
