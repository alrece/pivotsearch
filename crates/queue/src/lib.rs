//! # pivotsearch-queue
//!
//! 任务队列：单工作线程 + Task 状态机 + 多索引并发。
//!
//! 设计（复刻经典桌面搜索工具的 IndexingQueue 模式，净室重写）：
//! - 单工作线程串行执行（Tantivy 单 writer 强约束）
//! - Task 状态机：NotReady → Ready → Indexing → Finished
//! - UPDATE/REBUILD 语义
//! - 去重：队列里已有同 index_id 的 Ready Update → 新 Update 冗余丢弃
//! - SUCCESS_UNCHANGED 跳过持久化
//!
//! queue 只做调度，不依赖具体实现——通过 TaskHandler trait 执行任务。

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use pivotsearch_contracts::{IndexAction, Result, UpdateResult};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// 任务标识。
pub type TaskId = String;

/// Task 状态机。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    NotReady,
    Ready,
    Indexing,
    Finished,
}

/// 一个索引任务。
#[derive(Debug, Clone)]
pub struct Task {
    pub id: TaskId,
    pub index_id: String,
    pub action: IndexAction,
    pub state: TaskState,
}

impl Task {
    pub fn new(index_id: &str, action: IndexAction) -> Self {
        Self {
            id: format!("{}-{}", index_id, std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)),
            index_id: index_id.to_string(),
            action,
            state: TaskState::NotReady,
        }
    }

    pub fn update(index_id: &str) -> Self {
        Self::new(index_id, IndexAction::Update)
    }

    pub fn rebuild(index_id: &str) -> Self {
        Self::new(index_id, IndexAction::Rebuild)
    }
}

/// 任务处理器 trait。
///
/// queue 通过此 trait 调用实际的索引逻辑（由 cli/src-tauri 注入）。
/// queue 自身不依赖 index crate（依赖方向铁律）。
pub trait TaskHandler: Send + Sync {
    /// 执行一个任务，返回 UpdateResult。
    fn handle(&self, index_id: &str, action: IndexAction) -> Result<UpdateResult>;
}

/// 任务队列。
///
/// 单工作线程串行执行任务。线程安全（内部用 channel + Mutex）。
pub struct IndexingQueue {
    sender: Sender<Task>,
    /// 已入队但未完成的 index_id（去重用）。
    pending: Arc<Mutex<HashSet<String>>>,
    /// 工作线程句柄。
    worker: Mutex<Option<JoinHandle<()>>>,
    /// 停止标志。
    stop: Arc<AtomicBool>,
    /// 完成回调（可选）。
    on_complete: Arc<Mutex<Option<Box<dyn Fn(Task, UpdateResult) + Send + Sync>>>>,
}

impl IndexingQueue {
    /// 创建队列并启动工作线程。
    pub fn new(handler: Arc<dyn TaskHandler>) -> Self {
        let (sender, receiver) = unbounded::<Task>();
        let pending = Arc::new(Mutex::new(HashSet::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let on_complete: Arc<Mutex<Option<Box<dyn Fn(Task, UpdateResult) + Send + Sync>>>> =
            Arc::new(Mutex::new(None));

        let pending_clone = pending.clone();
        let stop_clone = stop.clone();
        let on_complete_clone = on_complete.clone();

        let worker = std::thread::spawn(move || {
            worker_loop(receiver, handler, pending_clone, stop_clone, on_complete_clone);
        });

        Self {
            sender,
            pending,
            worker: Mutex::new(Some(worker)),
            stop,
            on_complete,
        }
    }

    /// 设置任务完成回调（用于通知 UI 更新进度）。
    pub fn on_complete<F>(&self, callback: F)
    where
        F: Fn(Task, UpdateResult) + Send + Sync + 'static,
    {
        *self.on_complete.lock() = Some(Box::new(callback));
    }

    /// 入队一个任务。
    ///
    /// 去重：如果队列里已有同 index_id 的 Update 任务，新的 Update 被丢弃。
    /// Rebuild 不去重（强制重建）。
    pub fn enqueue(&self, mut task: Task) -> Result<()> {
        // 去重检查
        if task.action == IndexAction::Update {
            let mut pending = self.pending.lock();
            if pending.contains(&task.index_id) {
                tracing::debug!("冗余 Update 任务丢弃: {}", task.index_id);
                return Ok(());
            }
            pending.insert(task.index_id.clone());
        }
        task.state = TaskState::Ready;
        self.sender
            .send(task)
            .map_err(|e| pivotsearch_contracts::PivotsearchError::IndexIo(format!("enqueue: {e}")))
    }

    /// 便捷方法：入队一个 Update 任务。
    pub fn enqueue_update(&self, index_id: &str) -> Result<()> {
        self.enqueue(Task::update(index_id))
    }

    /// 便捷方法：入队一个 Rebuild 任务。
    pub fn enqueue_rebuild(&self, index_id: &str) -> Result<()> {
        self.enqueue(Task::rebuild(index_id))
    }

    /// 当前待处理任务数。
    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }

    /// 停止队列（等当前任务完成）。
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // 发送一个哨兵任务唤醒阻塞的 worker
        let _ = self.sender.send(Task::update("__shutdown__"));
        if let Some(worker) = self.worker.lock().take() {
            let _ = worker.join();
        }
    }
}

impl Drop for IndexingQueue {
    fn drop(&mut self) {
        if !self.stop.load(Ordering::SeqCst) {
            self.shutdown();
        }
    }
}

/// 工作线程循环：串行处理任务。
fn worker_loop(
    receiver: Receiver<Task>,
    handler: Arc<dyn TaskHandler>,
    pending: Arc<Mutex<HashSet<String>>>,
    stop: Arc<AtomicBool>,
    on_complete: Arc<Mutex<Option<Box<dyn Fn(Task, UpdateResult) + Send + Sync>>>>,
) {
    while !stop.load(Ordering::SeqCst) {
        let task = match receiver.recv() {
            Ok(t) => t,
            Err(_) => break, // sender 断开
        };

        // 哨兵任务
        if task.index_id == "__shutdown__" {
            break;
        }

        tracing::info!("处理任务: {} {:?} on {}", task.id, task.action, task.index_id);

        let result = handler.handle(&task.index_id, task.action);

        // 从 pending 移除
        pending.lock().remove(&task.index_id);

        match &result {
            Ok(update_result) => {
                tracing::info!(
                    "任务完成: {} → {:?}",
                    task.id, update_result
                );
                if let Some(cb) = on_complete.lock().as_ref() {
                    cb(task, *update_result);
                }
            }
            Err(e) => {
                tracing::error!("任务失败: {} → {}", task.id, e);
                if let Some(cb) = on_complete.lock().as_ref() {
                    cb(task, UpdateResult::Failure);
                }
            }
        }
    }
    tracing::info!("工作线程退出");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// 测试用 handler：记录调用次数。
    struct CountingHandler {
        count: Arc<AtomicUsize>,
    }

    impl TaskHandler for CountingHandler {
        fn handle(&self, _index_id: &str, _action: IndexAction) -> Result<UpdateResult> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(UpdateResult::SuccessChanged)
        }
    }

    #[test]
    fn queue_executes_tasks_serially() {
        let count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler { count: count.clone() });
        let queue = IndexingQueue::new(handler);

        queue.enqueue_update("idx-1").unwrap();
        queue.enqueue_update("idx-2").unwrap();
        queue.enqueue_update("idx-3").unwrap();

        // 等待任务完成
        std::thread::sleep(std::time::Duration::from_millis(500));

        assert_eq!(count.load(Ordering::SeqCst), 3, "应执行 3 个任务");
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn queue_dedup_redundant_updates() {
        let count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler { count: count.clone() });
        let queue = IndexingQueue::new(handler);

        // 快速连续入队 5 个同 index_id 的 Update
        for _ in 0..5 {
            queue.enqueue_update("idx-1").unwrap();
        }

        std::thread::sleep(std::time::Duration::from_millis(500));

        // 由于去重，应只执行 1 次
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "同 index_id 的冗余 Update 应去重为 1 次"
        );
    }

    #[test]
    fn queue_on_complete_callback() {
        let handler = Arc::new(CountingHandler {
            count: Arc::new(AtomicUsize::new(0)),
        });
        let results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = results.clone();

        let queue = IndexingQueue::new(handler);
        queue.on_complete(move |_task, result| {
            results_clone.lock().push(result);
        });

        queue.enqueue_update("idx-1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));

        let r = results.lock();
        assert!(!r.is_empty(), "应有完成回调");
        assert_eq!(r[0], UpdateResult::SuccessChanged);
    }
}
