//! # pivotsearch CLI
//!
//! 开发期调试用的命令行入口。先于 Tauri UI 跑通核心闭环。
//!
//! Phase 1 (T3) 将实现：
//!   pivotsearch index <dir>   索引一个目录
//!   pivotsearch search <query> 搜索
//!
//! Phase 0 占位：仅打印版本信息，验证 workspace 编译。

fn main() -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    println!("pivotsearch v{version}");
    println!();
    println!("跨平台本地全文搜索（开发期 CLI，Phase 1 起可用）");
    println!();
    println!("即将支持的命令：");
    println!("  pivotsearch index  <dir>     索引一个目录");
    println!("  pivotsearch search <query>   搜索");
    println!("  pivotsearch watch  <dir>     监听目录变化（Phase 2）");
    println!("  pivotsearch status           查看索引状态");
    println!();
    println!("当前状态：Phase 0 脚手架，核心实现见 Phase 1。");
    Ok(())
}
