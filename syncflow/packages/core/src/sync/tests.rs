#[cfg(test)]
mod tests {
    use crate::sync::version_vector::*;

    #[test]
    fn test_version_vector_new() {
        let vv = VersionVector::new("device_a");
        assert_eq!(vv.get("device_a"), 0);
    }

    #[test]
    fn test_version_vector_increment() {
        let mut vv = VersionVector::new("device_a");
        vv.increment("device_a");
        assert_eq!(vv.get("device_a"), 1);
        vv.increment("device_a");
        assert_eq!(vv.get("device_a"), 2);
    }

    #[test]
    fn test_version_vector_concurrent_means_conflict() {
        let mut vv_a = VersionVector::new("device_a");
        vv_a.increment("device_a");

        let mut vv_b = VersionVector::new("device_b");
        vv_b.increment("device_b");

        assert!(vv_a.is_conflicting(&vv_b));
        assert!(vv_b.is_conflicting(&vv_a));
    }

    #[test]
    fn test_version_vector_causally_ordered() {
        let mut vv_a = VersionVector::new("device_a");
        vv_a.increment("device_a");

        let mut vv_b = vv_a.clone();
        vv_b.increment("device_b");

        assert!(!vv_a.is_conflicting(&vv_b));
        assert!(vv_b.is_newer_than(&vv_a));
    }

    #[test]
    fn test_version_vector_serialize() {
        let mut vv = VersionVector::new("device_a");
        vv.increment("device_a");
        let json = vv.to_json().unwrap();
        let restored = VersionVector::from_json(&json).unwrap();
        assert_eq!(vv.get("device_a"), restored.get("device_a"));
    }

    #[test]
    fn test_file_event_classify() {
        use crate::sync::watcher::FileEvent;
        use std::path::PathBuf;

        let create_event = FileEvent::Created(PathBuf::from("/test/new.txt"));
        assert_eq!(create_event.path(), "/test/new.txt");

        let modify_event = FileEvent::Modified(PathBuf::from("/test/existing.txt"));
        assert_eq!(modify_event.path(), "/test/existing.txt");

        let delete_event = FileEvent::Deleted(PathBuf::from("/test/removed.txt"));
        assert_eq!(delete_event.path(), "/test/removed.txt");
    }
}
