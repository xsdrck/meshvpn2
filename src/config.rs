//! Configuration management for MeshVPN

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use crate::crypto::PublicKey;
use crate::errors::{Error, Result};

/// Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node name
    pub name: String,
    /// Listen address
    pub listen_addr: SocketAddr,
    /// Virtual IP address
    pub virtual_ip: IpAddr,
    /// Virtual network CIDR
    pub virtual_cidr: u8,
    /// Public key of this node
    pub public_key: PublicKey,
    /// Private key of this node (not serialized)
    #[serde(skip)]
    pub private_key: Option<crate::crypto::SecretKey>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            name: "mesh-node".to_string(),
            listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 51820),
            virtual_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            virtual_cidr: 24,
            public_key: PublicKey::from_bytes([0u8; 32]),
            private_key: None,
        }
    }
}

/// Peer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Peer name
    pub name: Option<String>,
    /// Public key
    pub public_key: PublicKey,
    /// Endpoint (if known)
    pub endpoint: Option<SocketAddr>,
    /// Allowed IPs
    pub allowed_ips: Vec<(IpAddr, u8)>,
    /// Persistent keepalive interval (seconds)
    pub persistent_keepalive: Option<u16>,
}

/// Signaling server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalingConfig {
    /// Signaling server address
    pub server_addr: SocketAddr,
    /// Whether this node is a signaling server
    pub is_server: bool,
}

impl Default for SignalingConfig {
    fn default() -> Self {
        Self {
            server_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            is_server: false,
        }
    }
}

/// STUN server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StunConfig {
    /// STUN server addresses
    pub servers: Vec<SocketAddr>,
    /// Enable STUN
    pub enabled: bool,
}

impl Default for StunConfig {
    fn default() -> Self {
        Self {
            servers: vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(142, 250, 193, 123)), 3478), // stun.l.google.com
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(142, 250, 193, 127)), 3478), // stun1.l.google.com
            ],
            enabled: true,
        }
    }
}

/// TUN device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunConfig {
    /// TUN device name
    pub name: Option<String>,
    /// Enable TUN device
    pub enabled: bool,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            name: Some("mesh0".to_string()),
            enabled: true,
        }
    }
}

/// Main configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Node configuration
    pub node: NodeConfig,
    /// Peers
    pub peers: Vec<PeerConfig>,
    /// Signaling configuration
    pub signaling: SignalingConfig,
    /// STUN configuration
    pub stun: StunConfig,
    /// TUN configuration
    pub tun: TunConfig,
    /// Configuration file path
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig::default(),
            peers: Vec::new(),
            signaling: SignalingConfig::default(),
            stun: StunConfig::default(),
            tun: TunConfig::default(),
            config_path: None,
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;
        
        let mut config: Self = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse config file: {}", e)))?;
        
        config.config_path = Some(path.clone());
        Ok(config)
    }

    /// Save configuration to a file
    pub fn save(&self) -> Result<()> {
        let path = self.config_path.as_ref()
            .ok_or_else(|| Error::Config("No config path set".to_string()))?;
        
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {}", e)))?;
        
        std::fs::write(path, content)
            .map_err(|e| Error::Config(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }

    /// Generate a new configuration with keys
    pub fn generate() -> Result<Self> {
        let keypair = crate::crypto::KeyPair::generate();
        let mut config = Self::default();
        
        config.node.public_key = *keypair.public_key();
        config.node.private_key = Some(keypair.secret_key().clone());
        
        Ok(config)
    }
}
