//! 引擎全局状态（跨命令共享）。

use std::collections::HashMap;

/// 引擎状态：管理所有索引根的 searcher 和 tree_index。
///
/// Phase 4 MVP：基础结构。T11 完善时接入完整的索引管理。
pub struct EngineState {
    /// index_id → 索引目录路径。
    pub index_dirs: HashMap<String, std::path::PathBuf>,
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            index_dirs: HashMap::new(),
        }
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

