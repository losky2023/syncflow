use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionVector {
    versions: HashMap<String, u64>,
    pub timestamp: DateTime<Utc>,
}

impl VersionVector {
    pub fn new(device_id: &str) -> Self {
        let mut versions = HashMap::new();
        versions.insert(device_id.to_string(), 0);
        Self {
            versions,
            timestamp: Utc::now(),
        }
    }

    pub fn increment(&mut self, device_id: &str) {
        let entry = self.versions.entry(device_id.to_string()).or_insert(0);
        *entry += 1;
        self.timestamp = Utc::now();
    }

    pub fn get(&self, device_id: &str) -> u64 {
        self.versions.get(device_id).copied().unwrap_or(0)
    }

    pub fn merge(&mut self, other: &VersionVector) {
        for (device_id, version) in &other.versions {
            let entry = self.versions.entry(device_id.clone()).or_insert(0);
            *entry = (*entry).max(*version);
        }
        self.timestamp = Utc::now();
    }

    pub fn is_conflicting(&self, other: &VersionVector) -> bool {
        let self_newer = self.is_newer_than(other);
        let other_newer = other.is_newer_than(self);
        !self_newer && !other_newer && self.versions != other.versions
    }

    pub fn is_newer_than(&self, other: &VersionVector) -> bool {
        for (device_id, version) in &other.versions {
            if self.get(device_id) < *version {
                return false;
            }
        }
        for (device_id, version) in &self.versions {
            if other.get(device_id) < *version {
                return true;
            }
        }
        false
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::error::SyncFlowError::Crypto(format!("VersionVector serialization: {}", e))
        })
    }

    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| {
            crate::error::SyncFlowError::Crypto(format!("VersionVector deserialization: {}", e))
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictStatus {
    IncomingNewer,
    LocalNewer,
    Conflict {
        local_version: VersionVector,
        incoming_version: VersionVector,
    },
}
