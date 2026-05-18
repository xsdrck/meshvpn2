use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use parking_lot::RwLock;
use tracing::{info, debug, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateType {
    Host,
    Srflx,
    Prflx,
    Relay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCandidate {
    pub foundation: String,
    pub component: u16,
    pub candidate_type: CandidateType,
    pub protocol: String,
    pub priority: u32,
    pub connection_address: IpAddr,
    pub connection_port: u16,
    pub generation: u32,
}

impl IceCandidate {
    pub fn new_host(ip: IpAddr, port: u16, component: u16) -> Self {
        Self {
            foundation: format!("1{}", component),
            component,
            candidate_type: CandidateType::Host,
            protocol: "UDP".to_string(),
            priority: calculate_priority(CandidateType::Host, 65535, component),
            connection_address: ip,
            connection_port: port,
            generation: 0,
        }
    }
    
    pub fn new_srflx(ip: IpAddr, port: u16, component: u16, base_priority: u32) -> Self {
        Self {
            foundation: format!("2{}", component),
            component,
            candidate_type: CandidateType::Srflx,
            protocol: "UDP".to_string(),
            priority: calculate_priority(CandidateType::Srflx, base_priority, component),
            connection_address: ip,
            connection_port: port,
            generation: 0,
        }
    }
    
    pub fn new_relay(ip: IpAddr, port: u16, component: u16) -> Self {
        Self {
            foundation: format!("3{}", component),
            component,
            candidate_type: CandidateType::Relay,
            protocol: "UDP".to_string(),
            priority: calculate_priority(CandidateType::Relay, 65535, component),
            connection_address: ip,
            connection_port: port,
            generation: 0,
        }
    }
    
    pub fn to_sdp_string(&self) -> String {
        format!(
            "candidate:{} {} {} {} {} {} typ {}",
            self.foundation,
            self.component,
            self.protocol.to_lowercase(),
            self.priority,
            self.connection_address,
            self.connection_port,
            match self.candidate_type {
                CandidateType::Host => "host",
                CandidateType::Srflx => "srflx",
                CandidateType::Prflx => "prflx",
                CandidateType::Relay => "relay",
            }
        )
    }
}

fn calculate_priority(candidate_type: CandidateType, local_pref: u32, component: u16) -> u32 {
    let type_pref = match candidate_type {
        CandidateType::Host => 126,
        CandidateType::Srflx => 100,
        CandidateType::Prflx => 110,
        CandidateType::Relay => 0,
    };
    
    (type_pref << 24) | (local_pref << 8) | (256 - component as u32)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IceCredentials {
    pub username: String,
    pub password: String,
}

impl IceCredentials {
    pub fn new() -> Self {
        use rand::RngCore;
        let mut username = [0u8; 16];
        let mut password = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut username);
        rand::rngs::OsRng.fill_bytes(&mut password);
        
        Self {
            username: base64_encode(&username),
            password: base64_encode(&password),
        }
    }
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    
    for chunk in data.chunks(3) {
        let b = match chunk.len() {
            1 => [chunk[0], 0, 0],
            2 => [chunk[0], chunk[1], 0],
            _ => [chunk[0], chunk[1], chunk[2]],
        };
        
        result.push(ALPHABET[(b[0] >> 2) as usize] as char);
        result.push(ALPHABET[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        
        if chunk.len() > 1 {
            result.push(ALPHABET[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        
        if chunk.len() > 2 {
            result.push(ALPHABET[(b[2] & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }
    
    result
}

pub struct IceAgent {
    local_credentials: IceCredentials,
    remote_credentials: Option<IceCredentials>,
    candidates: Arc<RwLock<Vec<IceCandidate>>>,
    local_ufrag: String,
    local_password: String,
}

impl IceAgent {
    pub fn new() -> Self {
        let creds = IceCredentials::new();
        Self {
            local_credentials: creds.clone(),
            remote_credentials: None,
            candidates: Arc::new(RwLock::new(Vec::new())),
            local_ufrag: creds.username,
            local_password: creds.password,
        }
    }
    
    pub fn get_credentials(&self) -> IceCredentials {
        self.local_credentials.clone()
    }
    
    pub fn set_remote_credentials(&mut self, creds: IceCredentials) {
        self.remote_credentials = Some(creds);
    }
    
    pub fn add_candidate(&self, candidate: IceCandidate) {
        self.candidates.write().push(candidate);
    }
    
    pub fn get_candidates(&self) -> Vec<IceCandidate> {
        self.candidates.read().clone()
    }
    
    pub async fn gather_candidates(&self, local_addr: SocketAddr) -> Result<Vec<IceCandidate>> {
        info!("Starting ICE candidate gathering for {}", local_addr);
        let mut candidates = Vec::new();
        
        let host_candidate = IceCandidate::new_host(
            local_addr.ip(),
            local_addr.port(),
            1
        );
        info!("Discovered host candidate: {}", host_candidate.to_sdp_string());
        candidates.push(host_candidate);
        self.add_candidate(candidates.last().unwrap().clone());
        
        match self.discover_srflx_candidates(local_addr).await {
            Ok(srflx_candidates) => {
                for c in srflx_candidates {
                    info!("Discovered srflx candidate: {}", c.to_sdp_string());
                    candidates.push(c.clone());
                    self.add_candidate(c);
                }
            }
            Err(e) => {
                warn!("Failed to discover srflx candidates: {}", e);
            }
        }
        
        Ok(candidates)
    }
    
    async fn discover_srflx_candidates(&self, local_addr: SocketAddr) -> Result<Vec<IceCandidate>> {
        use crate::nat::StunServer;
        
        let stun_server = StunServer::new("stun.l.google.com:19302".parse().unwrap());
        let response = stun_server.query(local_addr).await?;
        
        if response.mapped_address.ip() != local_addr.ip() 
           || response.mapped_address.port() != local_addr.port() {
            let base_priority = 65535;
            return Ok(vec![IceCandidate::new_srflx(
                response.mapped_address.ip(),
                response.mapped_address.port(),
                1,
                base_priority,
            )]);
        }
        
        Ok(Vec::new())
    }
    
    pub async fn try_connect(&self, remote_candidate: &IceCandidate) -> Result<bool> {
        info!("Attempting connection to {}", remote_candidate.connection_address);
        
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let local_addr = socket.local_addr()?;
        
        let binding_request = self.create_binding_request();
        socket.send_to(&binding_request, 
            SocketAddr::new(remote_candidate.connection_address, remote_candidate.connection_port)
        ).await?;
        
        let mut buf = [0u8; 1024];
        match timeout(Duration::from_secs(5), socket.recv_from(&mut buf)).await {
            Ok(Ok((_, _))) => {
                info!("Successfully connected to remote candidate");
                Ok(true)
            }
            Ok(Err(e)) => {
                error!("Connection failed: {}", e);
                Err(e.into())
            }
            Err(_) => {
                warn!("Connection timed out");
                Ok(false)
            }
        }
    }
    
    fn create_binding_request(&self) -> Vec<u8> {
        use rand::RngCore;
        let mut tid = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut tid);
        
        let mut msg = Vec::with_capacity(100);
        msg.extend_from_slice(&[0x00, 0x01]);
        msg.extend_from_slice(&(20u16).to_be_bytes());
        msg.extend_from_slice(&tid);
        
        msg.extend_from_slice(&[0x00, 0x06]);
        msg.extend_from_slice(&(self.local_ufrag.len() as u16).to_be_bytes());
        msg.extend_from_slice(self.local_ufrag.as_bytes());
        while msg.len() % 4 != 0 {
            msg.push(0);
        }
        
        msg
    }
    
    pub async fn perform_connectivity_check(
        &self,
        local_socket: &UdpSocket,
        remote: &IceCandidate,
    ) -> Result<bool> {
        let binding_req = self.create_binding_request();
        
        let remote_addr = SocketAddr::new(remote.connection_address, remote.connection_port);
        local_socket.send_to(&binding_req, remote_addr).await?;
        
        let mut buf = [0u8; 1024];
        match timeout(Duration::from_secs(3), local_socket.recv_from(&mut buf)).await {
            Ok(Ok(_)) => {
                info!("Connectivity check succeeded for {}", remote_addr);
                Ok(true)
            }
            _ => {
                debug!("Connectivity check failed for {}", remote_addr);
                Ok(false)
            }
        }
    }
    
    pub fn select_best_candidate(&self, remote_candidates: &[IceCandidate]) -> Option<IceCandidate> {
        let candidates_guard = self.candidates.read();
        let mut sorted: Vec<_> = candidates_guard.iter()
            .filter(|c| c.candidate_type != CandidateType::Host)
            .collect();
        
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        for local in sorted {
            for remote in remote_candidates {
                if local.candidate_type == remote.candidate_type {
                    return Some(remote.clone());
                }
            }
        }
        
        remote_candidates.first().cloned()
    }
}

pub struct IceLiteAgent {
    agent: IceAgent,
}

impl IceLiteAgent {
    pub fn new() -> Self {
        Self {
            agent: IceAgent::new(),
        }
    }
    
    pub async fn gather_candidates(&self, local_addr: SocketAddr) -> Result<Vec<IceCandidate>> {
        self.agent.gather_candidates(local_addr).await
    }
    
    pub fn get_credentials(&self) -> IceCredentials {
        self.agent.get_credentials()
    }
    
    pub fn get_local_ufrag(&self) -> &str {
        &self.agent.local_ufrag
    }
    
    pub fn get_local_password(&self) -> &str {
        &self.agent.local_password
    }
}
