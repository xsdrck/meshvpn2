use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use anyhow::{Result, Context};
use bytes::{BufMut, BytesMut};
use tracing::{info, debug, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatType {
    OpenInternet,
    FullCone,
    RestrictedCone,
    PortRestrictedCone,
    Symmetric,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct StunServer {
    address: SocketAddr,
}

impl StunServer {
    pub fn new(address: SocketAddr) -> Self {
        Self { address }
    }

    pub async fn query(&self, local_addr: SocketAddr) -> Result<StunResponse> {
        let socket = UdpSocket::bind(local_addr).await
            .with_context(|| format!("Failed to bind to {}", local_addr))?;
        
        let mut request = BytesMut::with_capacity(20);
        request.put_u16(0x0001);
        request.put_u16(0x0000);
        let tid = generate_transaction_id();
        request.put(&tid[..]);
        
        info!("Sending STUN binding request to {}", self.address);
        socket.send_to(&request, self.address).await?;
        
        let mut response_buf = vec![0u8; 1024];
        let (bytes_read, from) = timeout(Duration::from_secs(5), socket.recv_from(&mut response_buf))
            .await?
            .with_context(|| "STUN request timed out")?;
        
        debug!("Received {} bytes from {}", bytes_read, from);
        self.parse_response(&response_buf[..bytes_read])
    }

    fn parse_response(&self, data: &[u8]) -> Result<StunResponse> {
        if data.len() < 20 {
            anyhow::bail!("Response too short");
        }
        
        let msg_type = u16::from_be_bytes([data[0], data[1]]);
        let msg_length = u16::from_be_bytes([data[2], data[3]]);
        
        if msg_type != 0x0101 {
            anyhow::bail!("Not a STUN binding response");
        }
        
        let mut pos = 20;
        let mut mapped_addr = None;
        
        while pos < data.len() {
            if pos + 4 > data.len() {
                break;
            }
            
            let attr_type = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let attr_length = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            
            if pos + 4 + attr_length > data.len() {
                break;
            }
            
            if attr_type == 0x0020 {
                if attr_length >= 8 && data[pos + 5] == 0x01 {
                    let ip = format!("{}.{}.{}.{}",
                        data[pos + 8], data[pos + 9],
                        data[pos + 10], data[pos + 11]);
                    let port = u16::from_be_bytes([data[pos + 12], data[pos + 13]]);
                    mapped_addr = Some(SocketAddr::new(ip.parse()?, port));
                }
            }
            
            pos += 4 + attr_length;
            if attr_length % 4 != 0 {
                pos += 4 - (attr_length % 4);
            }
        }
        
        mapped_addr
            .map(StunResponse::from)
            .ok_or_else(|| anyhow::anyhow!("No MAPPED-ADDRESS found"))
    }
}

#[derive(Debug, Clone)]
pub struct StunResponse {
    pub mapped_address: SocketAddr,
    pub source_address: SocketAddr,
    pub changed_address: Option<SocketAddr>,
}

impl From<SocketAddr> for StunResponse {
    fn from(mapped_address: SocketAddr) -> Self {
        Self {
            mapped_address,
            source_address: mapped_address,
            changed_address: None,
        }
    }
}

pub struct NatDetector {
    stun_servers: Vec<StunServer>,
}

impl NatDetector {
    pub fn public() -> Self {
        Self {
            stun_servers: vec![
                StunServer::new("34.117.59.81:3478".parse().unwrap()),
                StunServer::new("34.117.118.251:3478".parse().unwrap()),
                StunServer::new("stun.l.google.com:19302".parse().unwrap()),
            ],
        }
    }
    
    pub async fn detect_nat_type(&self, local_addr: SocketAddr) -> Result<(NatType, SocketAddr)> {
        let mut last_error = None;
        
        for server in &self.stun_servers {
            match server.query(local_addr).await {
                Ok(response) => {
                    info!("STUN server {} returned: {:?}", server.address, response);
                    return Ok((NatType::Unknown, response.mapped_address));
                }
                Err(e) => {
                    warn!("STUN server {} failed: {}", server.address, e);
                    last_error = Some(e);
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All STUN servers failed")))
    }
    
    pub async fn test_nat_behavior(&self, local_addr: SocketAddr) -> Result<NatType> {
        let primary = StunServer::new("stun.l.google.com:3478".parse().unwrap());
        let resp1 = primary.query(local_addr).await?;
        
        let alt_socket = UdpSocket::bind(format!("{}:0", local_addr.ip())).await?;
        let alt_local = alt_socket.local_addr()?;
        
        let resp2 = primary.query(alt_local).await?;
        
        if resp1.mapped_address == resp2.mapped_address {
            if resp1.mapped_address == local_addr {
                Ok(NatType::OpenInternet)
            } else {
                Ok(NatType::FullCone)
            }
        } else {
            Ok(NatType::Symmetric)
        }
    }
}

fn generate_transaction_id() -> [u8; 12] {
    use rand::RngCore;
    let mut tid = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut tid);
    tid
}

pub async fn discover_public_endpoint(local_addr: SocketAddr) -> Result<SocketAddr> {
    let detector = NatDetector::public();
    let (_, public_addr) = detector.detect_nat_type(local_addr).await?;
    Ok(public_addr)
}
