use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::sync::Mutex;
use crate::sync::FileEvent;

#[derive(Debug, Clone)]
pub enum SyncTask {
    Upload { peer_id: String, path: PathBuf },
    Download { peer_id: String, path: PathBuf },
    Delete { peer_id: String, path: PathBuf },
}

pub struct SyncQueue {
    tasks: Mutex<VecDeque<SyncTask>>,
}

impl SyncQueue {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn enqueue(&self, event: &FileEvent, peer_ids: Vec<String>) {
        let mut tasks = self.tasks.lock().await;
        for peer_id in peer_ids {
            let task = match event {
                FileEvent::Created(path) | FileEvent::Modified(path) => {
                    SyncTask::Upload {
                        peer_id,
                        path: path.clone(),
                    }
                }
                FileEvent::Deleted(path) => {
                    SyncTask::Delete {
                        peer_id,
                        path: path.clone(),
                    }
                }
            };
            tasks.push_back(task);
        }
    }

    pub async fn dequeue(&self) -> Option<SyncTask> {
        self.tasks.lock().await.pop_front()
    }

    pub async fn is_empty(&self) -> bool {
        self.tasks.lock().await.is_empty()
    }

    pub async fn len(&self) -> usize {
        self.tasks.lock().await.len()
    }
}
