//! MeshVPN - A modern decentralized VPN with NAT traversal
//!
//! This is the main entry point for the MeshVPN application.

use anyhow::{Result, Context};
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use mesh_vpn::{
    Config, MeshNode, init_logging,
};

/// MeshVPN command-line interface
#[derive(Parser, Debug)]
#[command(name = "meshvpn")]
#[command(about = "A modern decentralized VPN with NAT traversal")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available commands
#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Generate a new configuration file
    Generate {
        /// Output file path
        #[arg(short, long, default_value = "meshvpn.toml")]
        output: PathBuf,
        /// Node name
        #[arg(short, long, default_value = "mesh-node")]
        name: String,
        /// Listen address
        #[arg(short, long, default_value = "0.0.0.0:51820")]
        listen: SocketAddr,
        /// Virtual IP address
        #[arg(short, long, default_value = "10.0.0.1")]
        virtual_ip: std::net::IpAddr,
    },
    /// Start the mesh node
    Start {
        /// Configuration file path
        #[arg(short, long, default_value = "meshvpn.toml")]
        config: PathBuf,
    },
    /// Show public key
    Pubkey {
        /// Configuration file path
        #[arg(short, long, default_value = "meshvpn.toml")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    init_logging();
    
    let cli = Cli::parse();
    
    match &cli.command {
        Commands::Generate { output, name, listen, virtual_ip } => {
            info!("Generating configuration file: {:?}", output);
            
            let mut config = Config::generate()?;
            config.node.name = name.clone();
            config.node.listen_addr = *listen;
            config.node.virtual_ip = *virtual_ip;
            config.config_path = Some(output.clone());
            
            // Extract the private key before saving
            let private_key = config.node.private_key.take();
            
            config.save()?;
            
            // Restore the private key and save separately
            if let Some(key) = private_key {
                let key_path = output.with_extension("key");
                std::fs::write(&key_path, hex::encode(key.as_bytes()))?;
                info!("Private key saved to: {:?}", key_path);
            }
            
            info!("Configuration generated successfully!");
            info!("Public key: {}", config.node.public_key.to_hex());
            Ok(())
        }
        Commands::Start { config } => {
            info!("Starting MeshVPN node with config: {:?}", config);
            
            let mut config = Config::load(config)
                .with_context(|| "Failed to load configuration")?;
            
            // Try to load private key from separate file if not in config
            if config.node.private_key.is_none() {
                if let Some(config_path) = &config.config_path {
                    let key_path = config_path.with_extension("key");
                    if key_path.exists() {
                        let key_hex = std::fs::read_to_string(&key_path)?;
                        let key_bytes = hex::decode(key_hex.trim())?;
                        let mut key_arr = [0u8; 32];
                        key_arr.copy_from_slice(&key_bytes);
                        config.node.private_key = Some(mesh_vpn::SecretKey::from_bytes(key_arr));
                        info!("Loaded private key from: {:?}", key_path);
                    }
                }
            }
            
            let mut node = MeshNode::new(config)
                .with_context(|| "Failed to create mesh node")?;
            
            info!("Node public key: {}", node.public_key().to_hex());
            
            node.start().await
                .with_context(|| "Failed to start mesh node")?;
            
            info!("MeshVPN node is running. Press Ctrl+C to stop.");
            
            // Wait for Ctrl+C
            tokio::signal::ctrl_c().await?;
            
            info!("Shutting down...");
            node.stop().await?;
            
            Ok(())
        }
        Commands::Pubkey { config } => {
            let config = Config::load(config)
                .with_context(|| "Failed to load configuration")?;
            println!("{}", config.node.public_key.to_hex());
            Ok(())
        }
    }
}
