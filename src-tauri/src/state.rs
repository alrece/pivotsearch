//! Global engine state (shared across commands).

use std::collections::HashMap;

/// Engine state: manages searchers and tree_index for all index roots.
///
/// Phase 4 MVP: foundational structure. Full index management is wired
/// up during T11 refinement.
pub struct EngineState {
    /// index_id -> index directory path.
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

