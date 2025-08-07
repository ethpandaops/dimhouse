//! Simplified initialization for xatu observer
//! This module consolidates all the initialization logic to minimize upstream code

use crate::chain::XatuChain as XatuChainNew;
use crate::config::NetworkInfo;
use crate::shim::create_exporter_with_network_info;
use crate::{XatuChain, XatuConfig};
use std::sync::Arc;
use tracing::{error, info};
use types::{ChainSpec, EthSpec};

/// Initialize xatu observer chain with minimal configuration
/// This handles all environment variable checking, config loading, and error handling
pub fn init<E: EthSpec>() -> Option<Arc<XatuChain<E>>> {
    info!("XATU FEATURE IS ENABLED - Initializing observer");

    // Check for XATU_CONFIG environment variable
    let config = if let Ok(config_path) = std::env::var("XATU_CONFIG") {
        info!("XATU_CONFIG env var found: {}", config_path);
        info!("Loading Xatu config from: {}", config_path);
        match XatuConfig::from_file(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!(
                    "Failed to load Xatu config: {}. Using default enabled config.",
                    e
                );
                XatuConfig::enabled()
            }
        }
    } else {
        // No config specified, check if we should still enable with defaults
        if std::env::var("DISABLE_XATU").is_ok() {
            info!("DISABLE_XATU set, xatu observer disabled");
            return None;
        }
        info!("No Xatu config specified, using default enabled config");
        XatuConfig::enabled()
    };

    if !config.is_enabled() {
        info!("Xatu is disabled in config");
        return None;
    }

    let exporter = crate::shim::create_exporter_from_config::<E>(&config)?;
    Some(Arc::new(XatuChainNew::with_exporter(exporter)))
}

/// Initialize xatu with chain spec
pub fn init_with_chain_spec<E: EthSpec>(
    spec: &ChainSpec,
) -> Result<Option<Arc<XatuChain<E>>>, String> {
    init_with_chain_spec_and_genesis::<E>(spec, spec.min_genesis_time)
}

/// Initialize xatu with chain spec and explicit genesis time
pub fn init_with_chain_spec_and_genesis<E: EthSpec>(
    spec: &ChainSpec,
    genesis_time: u64,
) -> Result<Option<Arc<XatuChain<E>>>, String> {
    info!("XATU FEATURE IS ENABLED - Initializing observer with chain spec");

    // Get config from environment or use defaults
    let config = if let Ok(config_path) = std::env::var("XATU_CONFIG") {
        info!("XATU_CONFIG env var found: {}", config_path);
        match XatuConfig::from_file(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!(
                    "Failed to load Xatu config: {}. Using default enabled config.",
                    e
                );
                XatuConfig::enabled()
            }
        }
    } else {
        if std::env::var("DISABLE_XATU").is_ok() {
            info!("DISABLE_XATU set, xatu observer disabled");
            return Ok(None);
        }
        info!("No Xatu config specified, using default enabled config");
        XatuConfig::enabled()
    };

    if !config.is_enabled() {
        info!("Xatu is disabled in config");
        return Ok(None);
    }

    // Determine network name - use override if provided, otherwise use chain spec
    let network_name = if let Some(ref ethereum_config) = config.ethereum {
        if let Some(ref override_name) = ethereum_config.override_network_name {
            info!("Using override network name from config: {}", override_name);
            override_name.clone()
        } else {
            spec.config_name
                .clone()
                .unwrap_or_else(|| "unknown".to_string())
        }
    } else {
        spec.config_name
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    };

    // Create network info from chain spec with explicit genesis time
    let network_info = NetworkInfo {
        genesis_time: genesis_time,
        network_name: network_name.clone(),
        network_id: spec.deposit_network_id,
        slots_per_epoch: E::slots_per_epoch(),
        seconds_per_slot: spec.seconds_per_slot,
    };

    info!(
        "Creating Xatu with network: {}, genesis_time: {} (actual)",
        network_info.network_name, network_info.genesis_time
    );

    // Create exporter with network info
    match create_exporter_with_network_info(&config, network_info) {
        Some(exporter) => Ok(Some(Arc::new(XatuChainNew::with_exporter(exporter)))),
        None => {
            // This should only happen if network info is missing or invalid
            Err("Failed to create Xatu exporter - network info may be invalid".to_string())
        }
    }
}
