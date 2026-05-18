//! TUN device interface
//!
//! This module provides a virtual network interface for sending and receiving IP packets.

use std::net::{IpAddr, Ipv4Addr};
use tracing::info;

use crate::errors::{Error, Result};

/// TUN device configuration
#[derive(Debug, Clone)]
pub struct TunDeviceConfig {
    /// Device name
    pub name: Option<String>,
    /// IP address
    pub address: IpAddr,
    /// Netmask
    pub netmask: IpAddr,
    /// MTU
    pub mtu: u16,
}

impl Default for TunDeviceConfig {
    fn default() -> Self {
        Self {
            name: Some("mesh0".to_string()),
            address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            netmask: IpAddr::V4(Ipv4Addr::new(255, 255, 255, 0)),
            mtu: 1420,
        }
    }
}

/// TUN device abstraction
#[derive(Debug, Clone)]
pub struct TunDevice {
    /// Device name
    name: String,
}

impl TunDevice {
    /// Create a new TUN device
    pub fn new(config: TunDeviceConfig) -> Result<Self> {
        let name = config.name.unwrap_or_else(|| "mesh0".to_string());
        
        info!("TUN device {} would be created (disabled in this build)", name);
        
        Ok(Self { name })
    }

    /// Get the device name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Read a packet from the TUN device
    pub async fn read_packet(&self) -> Option<Vec<u8>> {
        None
    }

    /// Write a packet to the TUN device
    pub async fn write_packet(&self, _packet: &[u8]) -> Result<()> {
        Ok(())
    }
}
