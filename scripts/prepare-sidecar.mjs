#!/usr/bin/env node
// 打包前编译 psearch CLI 并复制到 Tauri sidecar 期望位置。
// 跨平台（macOS/Linux/Windows），由 tauri.conf.json 的 beforeBundleCommand 调用。
// 用法: node scripts/prepare-sidecar.mjs

import { execSync } from "child_process";
import { copyFileSync, mkdirSync, existsSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

// 获取 target triple
const rustcV = execSync("rustc -vV").toString();
const targetMatch = rustcV.match(/host:\s*(.+)/);
const target = targetMatch ? targetMatch[1].trim() : process.platform + "-unknown";
const ext = process.platform === "win32" ? ".exe" : "";

console.log(`📦 编译 psearch CLI (target: ${target})...`);

// 编译 psearch
execSync("cargo build --release -p psearch", { cwd: root, stdio: "inherit" });

// 复制到 sidecar 位置
const src = join(root, "target", "release", `psearch${ext}`);
const binDir = join(root, "src-tauri", "binaries");
const dst = join(binDir, `psearch-${target}${ext}`);

if (!existsSync(src)) {
  console.error(`❌ 编译产物不存在: ${src}`);
  process.exit(1);
}

mkdirSync(binDir, { recursive: true });
copyFileSync(src, dst);
console.log(`✅ sidecar 就位: src-tauri/binaries/psearch-${target}${ext}`);
