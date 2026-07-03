# Refined Prompt — pivotsearch i18n + 文档英文为主

> 由 `/loop:refine` 生成。本文件是横切工具产物，不修改 `.loop/STATE.yaml`。

---

## 一、原始需求（原文）

> 软件应用好像忘记了一个重要的事项，忘记了中英文界面的切换能力；且既然开源使用，请也对所有的文档按github惯例，切换为英文版本为主，提供中文版查阅。请按要求修改后，进行再次提交。

## 二、追问记录（Q&A Log）

| # | 维度 | 用户选择 |
|---|------|---------|
| Q1 | i18n 范围 | **UI + CLI 加 `--lang` 参数**（UI 本地化；CLI 默认英文 JSON，支持 `--lang zh` 切中文供人类使用，Agent 不传则英文） |
| Q2 | 默认语言 | **跟随系统语言**（自动检测 OS locale：中文系统显中文，其他显英文；记忆用户手动切换） |
| Q3 | 文档范围 | **全部含代码注释**（README / CHANGELOG / AGENTS.md / CLAUDE.md + Rust/Vue 源码中文注释全转英文） |
| Q4 | 中文版格式 | **平行文件 `.zh-CN.md`**（README.md 英文 + README.zh-CN.md 中文，顶部互加语言切换链接） |

**自动判定（无需追问）：**
- ⑦ 向后兼容：i18n 不碰数据层 / 索引目录 / CLI JSON 协议（`--lang` 仅影响 human-readable 输出，JSON 输出固定英文 key）
- ⑧ 版本号：i18n 属新功能 → minor bump → **v0.5.0** + 发 Release（与 v0.4.0 同流程）

---

## 三、三套提示词

### 🅰️ 标准版（日常使用）

```
为 pivotsearch 添加中英文国际化能力，并把全部文档（含代码注释）切换为英文为主、中文版平行查阅。分两条工作线并行推进：

【工作线 A：应用 i18n（功能）】
A1. 前端 UI 本地化
  - 引入 vue-i18n（或等价方案），抽取 src/ 下所有中文字符串到 locale 资源文件
  - 产物：src/locales/en.ts 与 src/locales/zh-CN.ts
  - 顶部工具栏增加语言切换入口（中/EN 切换）；切换即时生效，无需重启
  - 默认语言：首次启动跟随系统语言（navigator.language / Tauri OS locale）；用户手动切换后写入 localStorage 持久化，下次启动沿用
A2. psearch CLI 增 --lang 参数
  - 默认英文（不破坏 AI Agent 的 JSON 解析稳定性，JSON 输出的 key 固定英文）
  - 支持 `--lang zh`：该参数仅影响 human-readable 输出（进度提示、状态信息、错误说明文本），JSON payload 内容保持不变
  - 在 crates/psearch/src/main.rs 用 clap 注册 --lang，落点到现有 eprintln/输出文本
A3. 语言切换不触碰数据层：索引目录、SQLite 元数据、CLI JSON 协议保持现状

【工作线 B：文档英文为主 + 中文平行版】
B1. 用户文档全部转英文为主
  - README.md / CHANGELOG.md / AGENTS.md / CLAUDE.md → 英文为主体
  - 中文版以平行文件提供：README.zh-CN.md / CHANGELOG.zh-CN.md / AGENTS.zh-CN.md / CLAUDE.zh-CN.md
  - 每个英文文档顶部加语言切换链接：`English | [中文](README.zh-CN.md)`，中文版反向链接
  - CHANGELOG：保留旧版本的中文历史记录原样，从 v0.5.0 起新条目用英文书写（英文版为权威版）
B2. 代码注释全部转英文
  - crates/ 与 src/ 与 src-tauri/src/ 下所有 Rust/Vue/TS 中文注释翻成英文，保留原意与代码风格
  - 净室红线不变：grep -ri "docfetcher" 仍必须 0 命中
  - 不改任何逻辑代码，仅注释与字符串文案

【交付与发版】
- 版本号 0.4.0 → 0.5.0（tauri.conf.json；CHANGELOG 加 v0.5.0 条目）
- 本地验证：cargo check --workspace + pnpm build + 手测语言切换
- 提交、打 tag v0.5.0、push 触发 Release CI，三端构建并发布
- 净室检查与依赖方向检查仍必须通过

执行顺序：A1/A2/B1/B2 可并行 → 本地验证 → commit → tag v0.5.0 → push → 监控 CI。
```

### 🅱️ 精简版（迭代对话/快速下发）

```
给 pivotsearch 做中英文国际化 + 文档英文为主：

1. 前端 i18n：vue-i18n，中文字符串抽到 src/locales/zh-CN.ts，新建 en.ts；顶栏加语言切换；默认跟随系统语言，手动切换后 localStorage 持久化
2. CLI i18n：psearch 加 `--lang`（默认英文，`--lang zh` 切中文；JSON 输出 key 固定英文，仅 human-readable 文案变）
3. 文档英文为主：README/CHANGELOG/AGENTS/CLAUDE 转英文，平行建 *.zh-CN.md，顶部互加语言链接
4. 代码注释全转英文（crates/ + src/ + src-tauri/src/），仅注释不动逻辑
5. 不碰数据层/索引/CLI JSON 协议；净室 grep 仍须 0 命中
6. 版本 0.4.0 → 0.5.0，CHANGELOG 加条目，本地验证后 commit → tag v0.5.0 → push 触发 Release CI → 监控三端构建。
```

### 🅲 高阶强约束版（适配 AI Agent）

```
任务：pivotsearch i18n + 文档英文为主（v0.4.0 → v0.5.0 + 发 Release）。
本任务受 AGENTS.md 约束（依赖方向铁律、净室红线、Tantivy 单 writer 约束），全程不得违反。

【强制前置】
- 先读 AGENTS.md（铁律 MUST/MUST NOT）、src-tauri/tauri.conf.json、crates/psearch/src/main.rs、src/App.vue、src/api.ts
- 先读 crates/contracts/src/lib.rs 确认 trait 边界，i18n 不得引入跨 crate 的耦合

【工作线 A：i18n（功能，不动数据层）】
A1. 前端本地化
  - 用 vue-i18n；所有 src/ 下中文 UI 字符串迁到 src/locales/{en.ts, zh-CN.ts}
  - 顶栏语言切换（中/EN），切换即时生效
  - 默认语言 = 首启动跟随 OS locale；用户切换后写 localStorage，下次启动优先 localStorage
  - 禁止把本地化 key 写进任何与索引/搜索/数据相关的代码路径
A2. psearch CLI 加 `--lang`（clap 注册）
  - 默认英文；`--lang zh` 仅影响 human-readable 文本（进度/状态/错误说明）
  - **铁律**：JSON 输出的 key 与结构固定英文，`--lang` 不得改变 JSON payload 的字段名/结构（保护 AI Agent 解析稳定性）
  - 落点：仅 crates/psearch 内的 eprintln/println 文本，不扩散到其他 crate
A3. CLI JSON 协议、SQLite schema、索引目录布局保持不变（向后兼容）

【工作线 B：文档与注释英文为主（不改逻辑）】
B1. README/CHANGELOG/AGENTS/CLAUDE 转英文为主
  - 平行中文版：*.zh-CN.md，顶部互加 `English | [中文](...)` 链接
  - CHANGELOG 旧版本中文记录保留原样，v0.5.0 起新条目英文（英文为权威）
B2. Rust/Vue/TS 中文注释全转英文（crates/ + src/ + src-tauri/src/）
  - 仅注释与显示文案，逻辑代码 0 行改动
  - **净室红线**：完成后必须跑 `grep -ri "docfetcher|net.sourceforge.docfetcher" crates/ src/ src-tauri/`，命中即未通过
  - **依赖方向红线**：core 不得 import 具体实现 crate（注释改动不得顺手引入 import）

【质量门（每项必须 PASS 才能进入下一步）】
1. cargo check --workspace 0 error
2. cargo clippy --all-targets -- -D warnings 0 warning（已有的 unused var 顺手修掉）
3. pnpm build 成功
4. 手测：启动 app，切中/英文都即时生效；psearch status（英文）/ psearch status --lang zh（中文）输出正确；psearch search "x" --json 在两种 --lang 下 JSON 结构一致
5. 净室 grep 0 命中
6. 依赖方向检查 0 命中

【交付】
- 版本号 0.4.0 → 0.5.0（tauri.conf.json + CHANGELOG v0.5.0 英文条目）
- commit → tag v0.5.0 → push → 监控 Release CI 三端 → 确认 Release 创建、5 个产物上传
- 产物清单核对：aarch64.dmg / amd64.AppImage / amd64.deb / x64-setup.exe / x64_en-US.msi
- 全程如遇与 AGENTS.md 铁律冲突，停止并报告，不得自行加 --force
```

---

## 四、选定版本

**🅰️ 标准版**（用户选定）

执行清单（按文件内标准版的执行顺序）：
- 工作线 A1：前端 vue-i18n 本地化（locale 资源 + 顶栏切换 + 跟随系统/记忆）
- 工作线 A2：psearch CLI 加 `--lang`（默认英文，`--lang zh` 切中文，JSON key 不变）
- 工作线 B1：README/CHANGELOG/AGENTS/CLAUDE 英文为主 + 平行 `.zh-CN.md`
- 工作线 B2：Rust/Vue/TS 代码注释全转英文（仅注释不动逻辑）
- 交付：版本 0.4.0 → 0.5.0，本地验证，commit，tag v0.5.0，push，监控 Release CI

> 注：refine 仅产出提示词，不推进 loop。是否执行由用户用 `/loop:run` 或直接下达执行指令决定。
