//! Mesh node implementation
//! 
//! This module provides the main MeshNode struct that ties all components together.

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;
use uuid::Uuid;

use crate::config::{Config, PeerConfig};
use crate::crypto::{KeyPair, PublicKey};
use crate::errors::{Error, Result};
use crate::tun::{TunDevice, TunDeviceConfig};
use crate::tunnel::wireguard::{WireGuardTunnel, WireGuardPeer};

/// Node state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// Node is starting
    Starting,
    /// Node is running
    Running,
    /// Node is stopping
    Stopping,
    /// Node is stopped
    Stopped,
}

/// Peer state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    /// Peer is disconnected
    Disconnected,
    /// Peer is connecting
    Connecting,
    /// Peer is connected
    Connected,
}

/// Peer information
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Public key
    pub public_key: PublicKey,
    /// Name
    pub name: Option<String>,
    /// State
    pub state: PeerState,
    /// Endpoint
    pub endpoint: Option<std::net::SocketAddr>,
    /// Last handshake time
    pub last_handshake: Option<std::time::Instant>,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
}

/// Mesh node - the main controller
pub struct MeshNode {
    /// Node ID
    id: Uuid,
    /// Configuration
    config: Arc<RwLock<Config>>,
    /// Key pair
    keypair: KeyPair,
    /// State
    state: Arc<RwLock<NodeState>>,
    /// Peers
    peers: Arc<RwLock<HashMap<PublicKey, PeerInfo>>>,
    /// WireGuard tunnel
    tunnel: Arc<RwLock<Option<WireGuardTunnel>>>,
    /// TUN device
    tun_device: Arc<RwLock<Option<TunDevice>>>,
}

impl MeshNode {
    /// Create a new mesh node
    pub fn new(mut config: Config) -> Result<Self> {
        let keypair = if let Some(private_key) = config.node.private_key.take() {
            KeyPair::from_secret_key(private_key)
        } else {
            return Err(Error::Config("No private key in config".to_string()));
        };
        
        Ok(Self {
            id: Uuid::new_v4(),
            config: Arc::new(RwLock::new(config)),
            keypair,
            state: Arc::new(RwLock::new(NodeState::Stopped)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            tunnel: Arc::new(RwLock::new(None)),
            tun_device: Arc::new(RwLock::new(None)),
        })
    }
    
    /// Get the node ID
    pub fn id(&self) -> Uuid {
        self.id
    }
    
    /// Get the node's public key
    pub fn public_key(&self) -> &PublicKey {
        self.keypair.public_key()
    }
    
    /// Get the current state
    pub async fn state(&self) -> NodeState {
        *self.state.read().await
    }
    
    /// Start the mesh node
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting mesh node: {}", self.id);
        
        {
            let mut state = self.state.write().await;
            *state = NodeState::Starting;
        }
        
        // Check if TUN is enabled
        let tun_enabled = {
            let config = self.config.read().await;
            config.tun.enabled
        };
        
        // Initialize WireGuard tunnel
        self.start_tunnel().await?;
        
        // Initialize TUN device if enabled
        if tun_enabled {
            self.start_tun_device().await?;
        }
        
        // Add peers from config
        self.initialize_peers().await?;
        
        {
            let mut state = self.state.write().await;
            *state = NodeState::Running;
        }
        
        info!("Mesh node started successfully");
        Ok(())
    }
    
    /// Stop the mesh node
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping mesh node: {}", self.id);
        
        {
            let mut state = self.state.write().await;
            *state = NodeState::Stopping;
        }
        
        // Stop tunnel
        {
            let mut tunnel = self.tunnel.write().await;
            *tunnel = None;
        }
        
        // Stop TUN device
        {
            let mut tun = self.tun_device.write().await;
            *tun = None;
        }
        
        {
            let mut state = self.state.write().await;
            *state = NodeState::Stopped;
        }
        
        info!("Mesh node stopped");
        Ok(())
    }
    
    /// Start the WireGuard tunnel
    async fn start_tunnel(&mut self) -> Result<()> {
        let config = self.config.read().await;
        
        let tunnel = WireGuardTunnel::new(
            config.node.listen_addr,
            Some(*self.keypair.secret_key().as_bytes()),
        ).await?;
        
        let mut tunnel_guard = self.tunnel.write().await;
        *tunnel_guard = Some(tunnel);
        
        info!("WireGuard tunnel started on {}", config.node.listen_addr);
        Ok(())
    }
    
    /// Start the TUN device
    async fn start_tun_device(&mut self) -> Result<()> {
        let config = self.config.read().await;
        
        let tun_config = TunDeviceConfig {
            name: config.tun.name.clone(),
            address: config.node.virtual_ip,
            netmask: match config.node.virtual_cidr {
                24 => std::net::IpAddr::V4(std::net::Ipv4Addr::new(255, 255, 255, 0)),
                _ => std::net::IpAddr::V4(std::net::Ipv4Addr::new(255, 255, 255, 0)),
            },
            mtu: 1420,
        };
        
        let tun = TunDevice::new(tun_config)?;
        
        let mut tun_guard = self.tun_device.write().await;
        *tun_guard = Some(tun);
        
        Ok(())
    }
    
    /// Initialize peers from config
    async fn initialize_peers(&self) -> Result<()> {
        let config = self.config.read().await;
        
        for peer_config in &config.peers {
            self.add_peer(peer_config.clone()).await?;
        }
        
        Ok(())
    }
    
    /// Add a peer
    pub async fn add_peer(&self, peer_config: PeerConfig) -> Result<()> {
        info!("Adding peer: {:?}", peer_config.name);
        
        // Create WireGuard peer
        let wg_peer = WireGuardPeer {
            public_key: *peer_config.public_key.as_bytes(),
            endpoint: peer_config.endpoint,
            allowed_ips: peer_config.allowed_ips.iter()
                .map(|(ip, cidr)| (ip.to_string(), *cidr as u32))
                .collect(),
            persistent_keepalive: peer_config.persistent_keepalive.unwrap_or(0),
        };
        
        // Add to tunnel
        if let Some(tunnel) = self.tunnel.read().await.as_ref() {
            tunnel.add_peer(wg_peer);
        }
        
        // Store peer info
        let peer_info = PeerInfo {
            public_key: peer_config.public_key,
            name: peer_config.name,
            state: PeerState::Disconnected,
            endpoint: peer_config.endpoint,
            last_handshake: None,
            bytes_sent: 0,
            bytes_received: 0,
        };
        
        self.peers.write().await.insert(peer_config.public_key, peer_info);
        
        Ok(())
    }
    
    /// Remove a peer
    pub async fn remove_peer(&self, public_key: &PublicKey) -> Result<()> {
        info!("Removing peer: {}", public_key.to_hex());
        
        // Remove from tunnel
        if let Some(tunnel) = self.tunnel.read().await.as_ref() {
            tunnel.remove_peer(public_key.as_bytes());
        }
        
        // Remove from peers map
        self.peers.write().await.remove(public_key);
        
        Ok(())
    }
    
    /// Get all peers
    pub async fn peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.values().cloned().collect()
    }
    
    /// Get a specific peer
    pub async fn get_peer(&self, public_key: &PublicKey) -> Option<PeerInfo> {
        self.peers.read().await.get(public_key).cloned()
    }
}
