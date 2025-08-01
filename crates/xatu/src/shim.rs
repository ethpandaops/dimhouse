//! Shim module for creating Xatu exporter

use crate::observer_ffi::XatuObserver;
use crate::Xatu;
use std::sync::Arc;
use types::EthSpec;

/// Create a default Xatu instance (always enabled)
pub fn create_exporter<E: EthSpec>() -> Arc<dyn Xatu<E>> {
    tracing::error!("Cannot create Xatu exporter without network info - this should not be called");
    panic!("Xatu requires network info to be initialized");
}

/// Create Xatu instance from configuration
pub fn create_exporter_from_config<E: EthSpec>(
    config: &crate::XatuConfig,
) -> Option<Arc<dyn Xatu<E>>> {
    if !config.is_enabled() {
        tracing::info!("Xatu is disabled");
        return None;
    }

    let full_config = config.get_full_config();
    match XatuObserver::new_with_full_config(&full_config, None) {
        Ok(middleware) => {
            tracing::info!("Xatu exporter created successfully with config");
            Some(Arc::new(middleware))
        }
        Err(e) => {
            tracing::error!("Failed to create Xatu: {}", e);
            panic!("Failed to initialize Xatu: {}", e);
        }
    }
}

/// Create Xatu instance with network info
pub fn create_exporter_with_network_info<E: EthSpec>(
    config: &crate::XatuConfig,
    network_info: crate::config::NetworkInfo,
) -> Option<Arc<dyn Xatu<E>>> {
    if !config.is_enabled() {
        tracing::info!("Xatu is disabled");
        return None;
    }

    let full_config = config.get_full_config();
    match XatuObserver::new_with_full_config(&full_config, Some(network_info)) {
        Ok(middleware) => Some(Arc::new(middleware)),
        Err(e) => {
            tracing::error!("FATAL: Failed to create Xatu with network info: {}", e);
            panic!("FATAL: Failed to initialize Xatu - network info is required but initialization failed: {}", e);
        }
    }
}
