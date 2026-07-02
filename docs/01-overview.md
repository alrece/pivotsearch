# 01 — 项目概述

## 一句话

跨平台本地全文搜索桌面应用，AnyTXT 的开源替代，用 Rust（Tantivy + Tauri）现代栈复刻 DocFetcher 核心设计并补齐 OCR。

## 问题陈述

本地全文搜索市场存在"三难困境"：

| 维度 | AnyTXT | DocFetcher 免费版 | Everything | Recoll |
|---|---|---|---|---|
| 开源 | ❌ 闭源 | ✅ GPL | ❌ 闭源 | ✅ GPL |
| 三端 | ❌ 仅 Win | ⚠️ Java 重 | ❌ 仅 Win | ⚠️ Linux 优先 |
| 内容搜索 | ✅ | ✅ | ❌ 仅文件名 | ✅ |
| 活跃维护 | ✅ 但退化 | ❌ 停滞 2023-10 | ✅ | ⚠️ |
| OCR | ✅ | ❌ 无 | ❌ | ⚠️ |

无工具同时满足"开源 + 三端 + 内容搜索 + 活跃 + OCR"。pivotsearch 填补空白。

## 现有工具短板（来自真实用户反馈）

**AnyTXT**（官方论坛）：
- GUI 启动慢
- 索引不完整/漏文档
- 几周后性能退化，唯一解法卸载重装
- 仅 Windows

**DocFetcher 免费版**：
- 实质停滞（最后大更新 2023-10）
- Java 重、吃内存
- 默认不自动索引需手动选目录
- 文件夹监听吃 CPU
- 无 OCR

## pivotsearch 定位

| 维度 | pivotsearch |
|---|---|
| 开源 | ✅ Apache-2.0 |
| 三端 | ✅ Tauri 2 原生 |
| 内容搜索 | ✅ Tantivy 全文 |
| 活跃 | ✅ 现代栈易维护 |
| OCR | ✅ Tesseract（可选） |
| 体积 | ✅ Rust + Tauri，无 JVM |

## 核心理念

1. **本地优先**：索引/搜索/OCR 全本地，零数据外泄（语言包下载除外）
2. **增量索引**：mtime + 文件树 diff，改动即更新
3. **多格式**：PDF/Office/MD/HTML/TXT/ePub/源码，纯 Rust 主体
4. **中文友好**：jieba 分词，处理 GBK/Big5 编码
5. **三端原生**：一份代码三端打包

## 非目标（v1 不做）

- 服务端/多用户协作（单机应用）
- 移动端（未来 Tauri mobile）
- 老格式 .doc/.ppt（无成熟纯 Rust 解析器）
- 云同步
