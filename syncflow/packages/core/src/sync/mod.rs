pub mod queue;
pub mod version_vector;
pub mod watcher;

#[cfg(test)]
mod tests;

pub use version_vector::{VersionVector, ConflictStatus};
