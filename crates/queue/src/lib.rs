//! # pivotsearch-queue
//!
//! 队列层：单工作线程 + Task 状态机 + 多索引并发。
//!
//! Phase 0 占位：具体实现见 Phase 2 (T6)。

// Phase 2 将实现：
// - task.rs    Task 状态机（NotReady→Ready→Indexing→Finished）+ UPDATE/REBUILD + 去重/重叠检测
// - worker.rs  单工作线程（crossbeam-channel 串行执行，Tantivy 单 writer 强约束）

pub use pivotsearch_contracts::{IndexAction};
