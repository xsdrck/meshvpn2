use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, RwLock, broadcast};
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use parking_lot::RwLock as PLRwLock;
use tracing::{info, debug, warn, error};
use uuid::Uuid;
use bytes::{BytesMut, BufMut};

use crate::nat::{IceAgent, IceCandidate, IceCredentials};
use crate::tunnel::{WireGuardPeer, WireGuardTunnel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalingMessage {
    Register { client_id: String, public_key: [u8; 32] },
    RegisterAck { success: bool },
    Offer { from: String, to: String, sdp: String },
    Answer { from: String, to: String, sdp: String },
    IceCandidate { from: String, to: String, candidate: IceCandidate },
    PeerList { peers: Vec<PeerInfo> },
    Ping,
    Pong,
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: String,
    pub public_key: [u8; 32],
    pub endpoint: Option<String>,
}

pub struct SignalingServer {
    address: SocketAddr,
    peers: Arc<PLRwLock<Vec<PeerInfo>>>,
    pending_offers: Arc<PLRwLock<Vec<(String, String, String)>>>,
}

impl SignalingServer {
    pub fn new(address: SocketAddr) -> Self {
        Self {
            address,
            peers: Arc::new(PLRwLock::new(Vec::new())),
            pending_offers: Arc::new(PLRwLock::new(Vec::new())),
        }
    }
    
    pub async fn start(&self) -> Result<()> {
        use tokio::net::TcpListener;
        
        let listener = TcpListener::bind(self.address).await
            .with_context(|| format!("Failed to bind to {}", self.address))?;
        
        info!("Signaling server listening on {}", self.address);
        
        let peers = self.peers.clone();
        let pending_offers = self.pending_offers.clone();
        
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from {}", addr);
                    
                    let peers = peers.clone();
                    let pending_offers = pending_offers.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, addr, peers, pending_offers).await {
                            error!("Client handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                }
            }
        }
    }
    
    async fn handle_client(
        stream: TcpStream,
        _addr: SocketAddr,
        peers: Arc<PLRwLock<Vec<PeerInfo>>>,
        pending_offers: Arc<PLRwLock<Vec<(String, String, String)>>>,
    ) -> Result<()> {
        let mut stream = stream;
        let mut buffer = vec![0u8; 65536];
        let mut client_id: Option<String> = None;
        let mut _public_key: Option<[u8; 32]> = None;
        
        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            
            match serde_json::from_slice::<SignalingMessage>(&buffer[..n]) {
                Ok(msg) => {
                    match msg {
                        SignalingMessage::Register { client_id: id, public_key: pk } => {
                            info!("Registering peer: {}", id);
                            client_id = Some(id.clone());
                            _public_key = Some(pk);
                            
                            let peer_info = PeerInfo {
                                id: id.clone(),
                                public_key: pk,
                                endpoint: None,
                            };
                            
                            peers.write().push(peer_info);
                            
                            let ack = SignalingMessage::RegisterAck { success: true };
                            let response = serde_json::to_vec(&ack)?;
                            stream.write_all(&response).await?;
                            
                            let peer_list = SignalingMessage::PeerList {
                                peers: peers.read().clone(),
                            };
                            let response = serde_json::to_vec(&peer_list)?;
                            stream.write_all(&response).await?;
                        }
                        
                        SignalingMessage::Offer { from, to, sdp } => {
                            info!("Offer from {} to {}", from, to);
                            pending_offers.write().push((from, to, sdp));
                        }
                        
                        SignalingMessage::Answer { from, to, sdp } => {
                            info!("Answer from {} to {}", from, to);
                            pending_offers.write().push((from, to, sdp));
                        }
                        
                        SignalingMessage::IceCandidate { from, to, candidate } => {
                            debug!("ICE candidate from {} to {}", from, to);
                            let msg = SignalingMessage::IceCandidate { from, to, candidate };
                            let _ = stream.write_all(&serde_json::to_vec(&msg)?).await;
                        }
                        
                        SignalingMessage::Ping => {
                            let pong = SignalingMessage::Pong;
                            let response = serde_json::to_vec(&pong)?;
                            stream.write_all(&response).await?;
                        }
                        
                        SignalingMessage::Error { message } => {
                            error!("Signaling error: {}", message);
                        }
                        
                        _ => {}
                    }
                }
                Err(e) => {
                    error!("Failed to parse message: {}", e);
                }
            }
        }
        
        if let Some(id) = client_id {
            peers.write().retain(|p| p.id != id);
            info!("Peer disconnected: {}", id);
        }
        
        Ok(())
    }
}

pub struct SignalingClient {
    server_address: SocketAddr,
    client_id: String,
    stream: Option<TcpStream>,
    public_key: [u8; 32],
    connected: Arc<PLRwLock<bool>>,
    event_sender: mpsc::UnboundedSender<SignalingEvent>,
    event_receiver: Arc<PLRwLock<Option<mpsc::UnboundedReceiver<SignalingEvent>>>>,
}

#[derive(Debug, Clone)]
pub enum SignalingEvent {
    PeerConnected(String),
    PeerDisconnected(String),
    OfferReceived { from: String, sdp: String },
    AnswerReceived { from: String, sdp: String },
    IceCandidateReceived { from: String, candidate: IceCandidate },
    Error(String),
}

impl SignalingClient {
    pub fn new(server_address: SocketAddr, public_key: [u8; 32]) -> Self {
        let client_id = Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::unbounded_channel();
        
        Self {
            server_address,
            client_id,
            stream: None,
            public_key,
            connected: Arc::new(PLRwLock::new(false)),
            event_sender: tx,
            event_receiver: Arc::new(PLRwLock::new(Some(rx))),
        }
    }
    
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to signaling server: {}", self.server_address);
        
        let stream = TcpStream::connect(self.server_address).await
            .with_context(|| format!("Failed to connect to {}", self.server_address))?;
        
        self.stream = Some(stream);
        *self.connected.write() = true;
        
        self.register().await?;
        
        let stream = self.stream.take().unwrap();
        let event_sender = self.event_sender.clone();
        let client_id = self.client_id.clone();
        let connected = self.connected.clone();
        
        tokio::spawn(async move {
            Self::read_loop(stream, event_sender, client_id, connected).await;
        });
        
        info!("Connected to signaling server");
        Ok(())
    }
    
    async fn register(&mut self) -> Result<()> {
        let msg = SignalingMessage::Register {
            client_id: self.client_id.clone(),
            public_key: self.public_key,
        };
        
        self.send_message(msg).await?;
        
        let mut buffer = vec![0u8; 65536];
        let stream = self.stream.as_mut().unwrap();
        let n = stream.read(&mut buffer).await?;
        
        let response: SignalingMessage = serde_json::from_slice(&buffer[..n])?;
        
        if let SignalingMessage::RegisterAck { success } = response {
            if success {
                info!("Successfully registered with ID: {}", self.client_id);
                Ok(())
            } else {
                anyhow::bail!("Registration failed")
            }
        } else {
            anyhow::bail!("Unexpected response")
        }
    }
    
    async fn send_message(&mut self, msg: SignalingMessage) -> Result<()> {
        let stream = self.stream.as_mut()
            .with_context(|| "Not connected")?;
        
        let data = serde_json::to_vec(&msg)?;
        stream.write_all(&data).await?;
        
        Ok(())
    }
    
    pub async fn send_offer(&mut self, to: &str, sdp: &str) -> Result<()> {
        info!("Sending offer to {}", to);
        
        let msg = SignalingMessage::Offer {
            from: self.client_id.clone(),
            to: to.to_string(),
            sdp: sdp.to_string(),
        };
        
        self.send_message(msg).await
    }
    
    pub async fn send_answer(&mut self, to: &str, sdp: &str) -> Result<()> {
        info!("Sending answer to {}", to);
        
        let msg = SignalingMessage::Answer {
            from: self.client_id.clone(),
            to: to.to_string(),
            sdp: sdp.to_string(),
        };
        
        self.send_message(msg).await
    }
    
    pub async fn send_ice_candidate(&mut self, to: &str, candidate: IceCandidate) -> Result<()> {
        let msg = SignalingMessage::IceCandidate {
            from: self.client_id.clone(),
            to: to.to_string(),
            candidate,
        };
        
        self.send_message(msg).await
    }
    
    pub fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<SignalingEvent>> {
        self.event_receiver.write().take()
    }
    
    async fn read_loop(
        mut stream: TcpStream,
        event_sender: mpsc::UnboundedSender<SignalingEvent>,
        _client_id: String,
        connected: Arc<PLRwLock<bool>>,
    ) {
        let mut buffer = vec![0u8; 65536];
        
        while *connected.read() {
            match stream.read(&mut buffer).await {
                Ok(0) => {
                    info!("Connection closed");
                    break;
                }
                Ok(n) => {
                    match serde_json::from_slice::<SignalingMessage>(&buffer[..n]) {
                        Ok(msg) => {
                            Self::handle_message(msg, &event_sender);
                        }
                        Err(e) => {
                            error!("Failed to parse message: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Read error: {}", e);
                    break;
                }
            }
        }
        
        *connected.write() = false;
    }
    
    fn handle_message(msg: SignalingMessage, sender: &mpsc::UnboundedSender<SignalingEvent>) {
        match msg {
            SignalingMessage::PeerList { peers } => {
                info!("Received peer list with {} peers", peers.len());
            }
            
            SignalingMessage::Offer { from, to: _, sdp } => {
                if let Err(e) = sender.send(SignalingEvent::OfferReceived { from, sdp }) {
                    error!("Failed to send event: {}", e);
                }
            }
            
            SignalingMessage::Answer { from, to: _, sdp } => {
                if let Err(e) = sender.send(SignalingEvent::AnswerReceived { from, sdp }) {
                    error!("Failed to send event: {}", e);
                }
            }
            
            SignalingMessage::IceCandidate { from, to: _, candidate } => {
                if let Err(e) = sender.send(SignalingEvent::IceCandidateReceived { from, candidate }) {
                    error!("Failed to send event: {}", e);
                }
            }
            
            SignalingMessage::PeerList { peers } => {
                for peer in peers {
                    if let Err(e) = sender.send(SignalingEvent::PeerConnected(peer.id)) {
                        error!("Failed to send event: {}", e);
                    }
                }
            }
            
            SignalingMessage::Error { message } => {
                if let Err(e) = sender.send(SignalingEvent::Error(message)) {
                    error!("Failed to send event: {}", e);
                }
            }
            
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct SdpMessage {
    pub sdp_type: SdpType,
    pub candidates: Vec<IceCandidate>,
    pub credentials: IceCredentials,
}

#[derive(Debug, Clone, Copy)]
pub enum SdpType {
    Offer,
    Answer,
}

impl SdpMessage {
    pub fn new_offer(ice_agent: &IceAgent) -> Self {
        Self {
            sdp_type: SdpType::Offer,
            candidates: ice_agent.get_candidates(),
            credentials: ice_agent.get_credentials(),
        }
    }
    
    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::json!({
            "type": match self.sdp_type {
                SdpType::Offer => "offer",
                SdpType::Answer => "answer",
            },
            "candidates": self.candidates,
            "ufrag": self.credentials.username,
            "password": self.credentials.password,
        });
        
        Ok(serde_json::to_string(&json)?)
    }
    
    pub fn from_json(json: &str) -> Result<Self> {
        let value: serde_json::Value = serde_json::from_str(json)?;
        
        let sdp_type = match value["type"].as_str().unwrap_or("") {
            "offer" => SdpType::Offer,
            "answer" => SdpType::Answer,
            _ => anyhow::bail!("Invalid SDP type"),
        };
        
        let candidates: Vec<IceCandidate> = serde_json::from_value(value["candidates"].clone())?;
        let credentials = IceCredentials {
            username: value["ufrag"].as_str().unwrap_or("").to_string(),
            password: value["password"].as_str().unwrap_or("").to_string(),
        };
        
        Ok(Self {
            sdp_type,
            candidates,
            credentials,
        })
    }
}
