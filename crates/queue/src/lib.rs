//! # pivotsearch-queue
//!
//! Task queue: single worker thread + Task state machine + multi-index concurrency.
//!
//! Design (replicates the IndexingQueue pattern of classic desktop search tools; clean-room rewrite):
//! - Single worker thread executes tasks serially (Tantivy single-writer constraint)
//! - Task state machine: NotReady → Ready → Indexing → Finished
//! - UPDATE/REBUILD semantics
//! - Deduplication: a new Update is dropped as redundant when a Ready Update with the same index_id is already in the queue
//! - SUCCESS_UNCHANGED skips persistence
//!
//! The queue only schedules and does not depend on concrete implementations — tasks are executed via the TaskHandler trait.

use crossbeam_channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use pivotsearch_contracts::{IndexAction, Result, UpdateResult};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Task identifier.
pub type TaskId = String;

/// Task state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    NotReady,
    Ready,
    Indexing,
    Finished,
}

/// An indexing task.
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

/// Task handler trait.
///
/// The queue invokes the actual indexing logic through this trait (injected by cli/src-tauri).
/// The queue itself does not depend on the index crate (strict dependency-direction rule).
pub trait TaskHandler: Send + Sync {
    /// Execute a task and return an UpdateResult.
    fn handle(&self, index_id: &str, action: IndexAction) -> Result<UpdateResult>;
}

/// Task queue.
///
/// A single worker thread executes tasks serially. Thread-safe (uses a channel + Mutex internally).
pub struct IndexingQueue {
    sender: Sender<Task>,
    /// Enqueued but not-yet-complete index_id (used for deduplication).
    pending: Arc<Mutex<HashSet<String>>>,
    /// Worker thread handle.
    worker: Mutex<Option<JoinHandle<()>>>,
    /// Stop flag.
    stop: Arc<AtomicBool>,
    /// Completion callback (optional).
    on_complete: Arc<Mutex<Option<Box<dyn Fn(Task, UpdateResult) + Send + Sync>>>>,
}

impl IndexingQueue {
    /// Create the queue and start the worker thread.
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

    /// Set the task-completion callback (used to notify the UI of progress updates).
    pub fn on_complete<F>(&self, callback: F)
    where
        F: Fn(Task, UpdateResult) + Send + Sync + 'static,
    {
        *self.on_complete.lock() = Some(Box::new(callback));
    }

    /// Enqueue a task.
    ///
    /// Deduplication: if an Update task with the same index_id is already in the queue, the new Update is dropped.
    /// Rebuild is not deduplicated (forces a rebuild).
    pub fn enqueue(&self, mut task: Task) -> Result<()> {
        // Deduplication check
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

    /// Convenience method: enqueue an Update task.
    pub fn enqueue_update(&self, index_id: &str) -> Result<()> {
        self.enqueue(Task::update(index_id))
    }

    /// Convenience method: enqueue a Rebuild task.
    pub fn enqueue_rebuild(&self, index_id: &str) -> Result<()> {
        self.enqueue(Task::rebuild(index_id))
    }

    /// Current number of pending tasks.
    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }

    /// Stop the queue (waits for the current task to finish).
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // Send a sentinel task to wake the blocked worker
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

/// Worker thread loop: processes tasks serially.
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
            Err(_) => break, // sender disconnected
        };

        // Sentinel task
        if task.index_id == "__shutdown__" {
            break;
        }

        tracing::info!("处理任务: {} {:?} on {}", task.id, task.action, task.index_id);

        let result = handler.handle(&task.index_id, task.action);

        // Remove from pending
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

    /// Test handler: records call count.
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

        // Wait for tasks to complete
        std::thread::sleep(std::time::Duration::from_millis(500));

        assert_eq!(count.load(Ordering::SeqCst), 3, "should execute 3 tasks");
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn queue_dedup_redundant_updates() {
        let count = Arc::new(AtomicUsize::new(0));
        let handler = Arc::new(CountingHandler { count: count.clone() });
        let queue = IndexingQueue::new(handler);

        // Rapidly enqueue 5 Updates with the same index_id in succession
        for _ in 0..5 {
            queue.enqueue_update("idx-1").unwrap();
        }

        std::thread::sleep(std::time::Duration::from_millis(500));

        // Due to deduplication, it should only execute once
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "redundant Updates with the same index_id should be deduplicated to 1"
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
        assert!(!r.is_empty(), "should have a completion callback");
        assert_eq!(r[0], UpdateResult::SuccessChanged);
    }
}
