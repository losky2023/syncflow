use mdns_sd::{ServiceDaemon, ServiceInfo};

const SERVICE_TYPE: &str = "_syncflow._tcp.local.";

/// A device discovered on the LAN via mDNS.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    pub platform: String,
}

impl DiscoveredDevice {
    /// Parse a DiscoveredDevice from mDNS service info.
    pub fn from_service_info(info: &mdns_sd::ServiceInfo) -> Option<Self> {
        // Extract device_id from the fullname: "instance_name._syncflow._tcp.local."
        let fullname = info.get_fullname();
        let ty_domain = info.get_type();
        let device_id = fullname
            .strip_suffix(&format!(".{}", ty_domain))
            .unwrap_or(fullname)
            .to_string();

        let device_name = info
            .get_property_val_str("device_name")
            .map(|s| s.to_string())
            .unwrap_or_else(|| device_id.clone());

        let platform = info
            .get_property_val_str("platform")
            .unwrap_or("unknown")
            .to_string();

        let ip = info
            .get_addresses()
            .iter()
            .next()
            .map(|a| a.to_string())
            .unwrap_or_default();

        let port = info.get_port();

        Some(Self {
            device_id,
            device_name,
            ip,
            port,
            platform,
        })
    }

    /// Build the base URL for this device's SDP exchange server.
    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.ip, self.port)
    }
}

/// mDNS discovery service that registers this device and browses for peers.
pub struct DiscoveryService {
    daemon: ServiceDaemon,
}

impl DiscoveryService {
    /// Register this device on the LAN and start browsing for peers.
    ///
    /// Returns the discovery service and a receiver for discovered devices.
    pub fn new(
        device_id: &str,
        device_name: &str,
        platform: &str,
        port: u16,
    ) -> Result<(Self, tokio::sync::mpsc::Receiver<DiscoveredDevice>), crate::error::SyncFlowError>
    {
        let daemon = ServiceDaemon::new().map_err(|e| {
            crate::error::SyncFlowError::Signal(format!("Failed to create mDNS daemon: {}", e))
        })?;

        // Register this device
        let props = [("device_name", device_name), ("platform", platform)];
        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            device_id,
            &format!("{}.local.", device_name),
            "127.0.0.1",
            port,
            &props[..],
        )
        .map_err(|e| {
            crate::error::SyncFlowError::Signal(format!(
                "Failed to create mDNS service info: {}",
                e
            ))
        })?;

        daemon.register(service_info).map_err(|e| {
            crate::error::SyncFlowError::Signal(format!("Failed to register mDNS service: {}", e))
        })?;

        // Start browsing
        let rx = daemon.browse(SERVICE_TYPE).map_err(|e| {
            crate::error::SyncFlowError::Signal(format!("Failed to start mDNS browse: {}", e))
        })?;

        // Spawn a task to convert mDNS events to discovered devices
        let (tx, local_rx) = tokio::sync::mpsc::channel(100);
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                    if let Some(device) = DiscoveredDevice::from_service_info(&info) {
                        let _ = tx.blocking_send(device);
                    }
                }
            }
        });

        Ok((Self { daemon }, local_rx))
    }

    /// Stop the discovery service.
    pub fn stop(self) {
        let _ = self.daemon.shutdown();
    }
}
