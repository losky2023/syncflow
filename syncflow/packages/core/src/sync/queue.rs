use crate::sync::FileEvent;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum SyncTask {
    Upload {
        peer_id: String,
        space_id: Uuid,
        relative_path: String,
        resolved_path: PathBuf,
    },
    Download {
        peer_id: String,
        space_id: Uuid,
        relative_path: String,
        resolved_path: PathBuf,
    },
    Delete {
        peer_id: String,
        space_id: Uuid,
        relative_path: String,
    },
}

pub struct SyncQueue {
    tasks: Mutex<VecDeque<SyncTask>>,
}

impl Default for SyncQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncQueue {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn enqueue(
        &self,
        space_id: Uuid,
        relative_path: &str,
        resolved_path: &Path,
        event: &FileEvent,
        peer_ids: Vec<String>,
    ) {
        let mut tasks = self.tasks.lock().await;
        for peer_id in peer_ids {
            let task = match event {
                FileEvent::Created(_) | FileEvent::Modified(_) => SyncTask::Upload {
                    peer_id,
                    space_id,
                    relative_path: relative_path.to_string(),
                    resolved_path: resolved_path.to_path_buf(),
                },
                FileEvent::Deleted(_) => SyncTask::Delete {
                    peer_id,
                    space_id,
                    relative_path: relative_path.to_string(),
                },
            };
            tasks.push_back(task);
        }
    }

    pub async fn dequeue(&self) -> Option<SyncTask> {
        self.tasks.lock().await.pop_front()
    }

    pub async fn requeue_front(&self, task: SyncTask) {
        self.tasks.lock().await.push_front(task);
    }

    pub async fn is_empty(&self) -> bool {
        self.tasks.lock().await.is_empty()
    }

    pub async fn len(&self) -> usize {
        self.tasks.lock().await.len()
    }
}
