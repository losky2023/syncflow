use crate::transport::discovery::{DiscoveredDevice, DiscoveryService};
use std::collections::HashMap;
use uuid::Uuid;

#[tokio::test]
async fn test_discovered_device_from_service_info() {
    let mut props = HashMap::new();
    props.insert("device_name".to_string(), "test-device".to_string());
    props.insert("platform".to_string(), "windows".to_string());

    let info = mdns_sd::ServiceInfo::new(
        "_syncflow._tcp.local.",
        "test-device-id",
        "test-device_device",
        "192.168.1.10",
        18080,
        props,
    )
    .unwrap();

    let device = DiscoveredDevice::from_service_info(&info).unwrap();
    assert_eq!(device.device_id, "test-device-id");
    assert_eq!(device.device_name, "test-device");
    assert_eq!(device.ip, "192.168.1.10");
    assert_eq!(device.port, 18080);
    assert_eq!(device.platform, "windows");
}

#[tokio::test]
async fn test_discovery_service_create_and_stop() {
    let device_id = Uuid::new_v4().to_string();
    let (service, _rx) = DiscoveryService::new(&device_id, "my-pc", "windows", 18080).unwrap();
    // Should not panic; service can be created and dropped cleanly
    drop(service);
}
