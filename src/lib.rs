//! MeshVPN - A modern decentralized VPN with NAT traversal
//!
//! This crate provides a complete VPN solution inspired by Tailscale, Zerotier, and WireGuard.
//!
//! # Features
//!
//! - NAT traversal (STUN/TURN/ICE)
//! - WireGuard-like encryption (ChaCha20Poly1305, X25519)
//! - Mesh networking with automatic discovery
//! - TUN/TAP virtual network interface
//! - HTTP API for management
//! - Decentralized routing
//!

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    unused_import_braces,
    unused_qualifications,
    missing_debug_implementations
)]

// Re-export important types
pub use config::{Config, NodeConfig, PeerConfig};
pub use crypto::{KeyPair, PublicKey, SecretKey};
pub use errors::{Error, Result};
pub use node::{MeshNode, NodeState, PeerInfo, PeerState};

// Core modules
pub mod config;
pub mod crypto;
pub mod errors;
pub mod node;

// Network modules
pub mod tunnel;
pub mod tun;

/// The version of the crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The user agent string used for networking
pub fn user_agent() -> String {
    format!("MeshVPN/{}", VERSION)
}

/// Initialize logging system
pub fn init_logging() {
    use tracing_subscriber::prelude::*;

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "meshvpn=info,warn".into());

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339())
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .try_init()
        .ok();
}
