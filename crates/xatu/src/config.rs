use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Network information passed from Lighthouse
#[derive(Debug, Clone, Serialize)]
pub struct NetworkInfo {
    pub genesis_time: u64,
    pub network_name: String,
    pub network_id: u64,
    pub slots_per_epoch: u64,
    pub seconds_per_slot: u64,
}

/// Simple Xatu configuration - just enabled/disabled
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct XatuConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<XatuOutput>>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ntpServer")]
    pub ntp_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethereum: Option<EthereumConfig>,
}

/// Node configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeConfig {
    pub name: String,
}

/// Ethereum configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct EthereumConfig {
    #[serde(
        rename = "overrideNetworkName",
        skip_serializing_if = "Option::is_none"
    )]
    pub override_network_name: Option<String>,
}

/// Output configuration wrapper
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct XatuOutput {
    pub name: String,
    #[serde(rename = "type")]
    pub output_type: String,
    pub config: OutputConfig,
}

/// Full configuration to pass to Go side
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FullConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeConfig>,
    pub outputs: Vec<XatuOutput>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ntpServer")]
    pub ntp_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethereum: Option<EthereumConfig>,
}

/// Output configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub address: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub tls: bool,
    #[serde(rename = "maxQueueSize", skip_serializing_if = "Option::is_none")]
    pub max_queue_size: Option<u64>,
    #[serde(rename = "batchTimeout", skip_serializing_if = "Option::is_none")]
    pub batch_timeout: Option<String>,
    #[serde(rename = "exportTimeout", skip_serializing_if = "Option::is_none")]
    pub export_timeout: Option<String>,
    #[serde(rename = "maxExportBatchSize", skip_serializing_if = "Option::is_none")]
    pub max_export_batch_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workers: Option<u64>,
}

/// Client information for Xatu
#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Network information for Ethereum
#[derive(Debug, Clone, Serialize)]
pub struct Network {
    pub name: String,
    pub id: u64,
}

/// Ethereum configuration for Xatu
#[derive(Debug, Clone, Serialize)]
pub struct XatuEthereum {
    pub genesis_time: u64,
    pub seconds_per_slot: u64,
    pub slots_per_epoch: u64,
    pub network: Network,
}

/// Xatu processor configuration
#[derive(Debug, Clone, Serialize)]
pub struct XatuProcessorConfig {
    pub name: String,
    pub outputs: Vec<XatuOutput>,
    pub ethereum: XatuEthereum,
    pub client: ClientInfo,
    #[serde(rename = "ntpServer", skip_serializing_if = "Option::is_none")]
    pub ntp_server: Option<String>,
}

/// Combined configuration to pass to FFI
#[derive(Debug, Clone, Serialize)]
pub struct FullConfigWithRuntime {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    pub processor: XatuProcessorConfig,
}

impl XatuConfig {
    /// Create an enabled configuration with default output
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            name: None,
            outputs: None,
            ntp_server: None,
            ethereum: None,
        }
    }

    /// Check if Xatu is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, String> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        serde_yaml::from_str(&contents).map_err(|e| format!("Failed to parse config file: {}", e))
    }

    /// Get a config structure that includes all outputs
    pub fn get_full_config(&self) -> FullConfig {
        // Create node config from the name field
        let node = self
            .name
            .as_ref()
            .map(|name| NodeConfig { name: name.clone() });

        FullConfig {
            node,
            outputs: self.outputs.clone().unwrap_or_default(),
            ntp_server: self.ntp_server.clone(),
            ethereum: self.ethereum.clone(),
        }
    }
}
