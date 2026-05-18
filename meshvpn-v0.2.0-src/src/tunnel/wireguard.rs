use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use anyhow::{Result, Context};
use bytes::{Bytes, BytesMut, BufMut};
use parking_lot::RwLock as PLRwLock;
use tracing::{info, debug, warn, error};
use rand::{RngCore, rngs::OsRng};

const WIREGUARD_HEADER_SIZE: usize = 32;
const WIREGUARD_MESSAGE_TYPES: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    HandshakeInitiation = 1,
    HandshakeResponse = 2,
    CookieReply = 3,
    TransportData = 4,
}

#[derive(Debug, Clone)]
pub struct HandshakeInitiation {
    pub sender_index: u32,
    pub ephemeral: [u8; 32],
    pub encrypted_static: Vec<u8>,
    pub encrypted_timestamp: Vec<u8>,
    pub mac1: [u8; 16],
    pub mac2: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct HandshakeResponse {
    pub sender_index: u32,
    pub receiver_index: u32,
    pub ephemeral: [u8; 32],
    pub encrypted_nothing: Vec<u8>,
    pub mac1: [u8; 16],
    pub mac2: [u8; 16],
}

#[derive(Debug, Clone)]
pub struct TransportMessage {
    pub receiver_index: u32,
    pub counter: u64,
    pub encrypted_packet: Vec<u8>,
}

#[derive(Clone)]
pub struct WireGuardPeer {
    pub public_key: [u8; 32],
    pub endpoint: Option<SocketAddr>,
    pub allowed_ips: Vec<(String, u32)>,
    pub persistent_keepalive: u16,
}

pub struct SessionKeys {
    pub sending: EncryptionKey,
    pub receiving: EncryptionKey,
    pub sending_key: [u8; 32],
    pub receiving_key: [u8; 32],
}

#[derive(Clone)]
pub struct EncryptionKey {
    pub key: [u8; 32],
    pub counter: u64,
    pub nonce: [u8; 12],
}

impl EncryptionKey {
    pub fn new(key: [u8; 32]) -> Self {
        Self {
            key,
            counter: 0,
            nonce: [0u8; 12],
        }
    }
    
    pub fn increment_counter(&mut self) {
        self.counter += 1;
        self.nonce[4..].copy_from_slice(&self.counter.to_le_bytes());
    }
}

pub struct WireGuardTunnel {
    socket: Arc<UdpSocket>,
    local_private_key: [u8; 32],
    local_public_key: [u8; 32],
    peers: Arc<PLRwLock<Vec<WireGuardPeer>>>,
    sessions: Arc<PLRwLock<Vec<Session>>>,
    next_sender_index: Arc<PLRwLock<u32>>,
    keepalive_sender: Arc<PLRwLock<Option<mpsc::Sender<()>>>>,
}

#[derive(Clone)]
struct Session {
    peer_index: u32,
    sending_key: EncryptionKey,
    receiving_key: EncryptionKey,
    last_used: Instant,
    local_index: u32,
    remote_index: u32,
}

impl WireGuardTunnel {
    pub async fn new(listen_addr: SocketAddr, private_key: Option<[u8; 32]>) -> Result<Self> {
        let socket = UdpSocket::bind(listen_addr).await
            .with_context(|| format!("Failed to bind to {}", listen_addr))?;
        
        let (private_key, public_key) = if let Some(key) = private_key {
            (key, derive_public_key(&key))
        } else {
            let private = generate_key();
            (private, derive_public_key(&private))
        };
        
        info!("WireGuard tunnel initialized");
        info!("Local public key: {}", hex_encode(&public_key));
        
        Ok(Self {
            socket: Arc::new(socket),
            local_private_key: private_key,
            local_public_key: public_key,
            peers: Arc::new(PLRwLock::new(Vec::new())),
            sessions: Arc::new(PLRwLock::new(Vec::new())),
            next_sender_index: Arc::new(PLRwLock::new(1)),
            keepalive_sender: Arc::new(PLRwLock::new(None)),
        })
    }
    
    pub fn local_public_key(&self) -> [u8; 32] {
        self.local_public_key
    }
    
    pub fn add_peer(&self, peer: WireGuardPeer) {
        info!("Adding peer with public key: {}", hex_encode(&peer.public_key));
        self.peers.write().push(peer);
    }
    
    pub fn remove_peer(&self, public_key: &[u8; 32]) {
        let mut peers = self.peers.write();
        peers.retain(|p| p.public_key != *public_key);
    }
    
    pub async fn initiate_handshake(&self, peer_public_key: &[u8; 32]) -> Result<()> {
        info!("Initiating handshake with peer: {}", hex_encode(peer_public_key));
        
        let peers_guard = self.peers.read();
        let peer = peers_guard.iter()
            .find(|p| p.public_key == *peer_public_key)
            .with_context(|| "Peer not found")?;
        
        let endpoint = peer.endpoint;
        drop(peers_guard);
        
        let mut initiation = HandshakeInitiation {
            sender_index: self.allocate_sender_index(),
            ephemeral: generate_key(),
            encrypted_static: Vec::new(),
            encrypted_timestamp: Vec::new(),
            mac1: [0u8; 16],
            mac2: [0u8; 16],
        };
        
        initiation.encrypted_static = self.encrypt_identity(peer_public_key)?;
        let timestamp = create_timestamp();
        initiation.encrypted_timestamp = self.encrypt_timestamp(&timestamp)?;
        
        let msg = self.encode_handshake_initiation(&initiation);
        
        if let Some(ep) = endpoint {
            self.socket.send_to(&msg, ep).await?;
            info!("Handshake initiation sent to {}", ep);
        }
        
        Ok(())
    }
    
    fn encrypt_identity(&self, _peer_public: &[u8; 32]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };
        
        let key = chacha20poly1305::Key::from_slice(&self.local_private_key);
        let cipher = ChaCha20Poly1305::new(key);
        
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce[4..]);
        
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), self.local_public_key.as_slice())
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        
        let mut result = nonce[4..].to_vec();
        result.extend(ciphertext);
        Ok(result)
    }
    
    fn encrypt_timestamp(&self, timestamp: &[u8]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };
        
        let key = chacha20poly1305::Key::from_slice(&self.local_private_key);
        let cipher = ChaCha20Poly1305::new(key);
        
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce[4..]);
        
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), timestamp)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        
        let mut result = nonce[4..].to_vec();
        result.extend(ciphertext);
        Ok(result)
    }
    
    fn encode_handshake_initiation(&self, msg: &HandshakeInitiation) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(148);
        
        buf.put_u8(MessageType::HandshakeInitiation as u8);
        buf.put_u8(0);
        buf.put_u16(28);
        buf.put_u32(msg.sender_index);
        buf.put(&msg.ephemeral[..]);
        
        buf.put_u16(msg.encrypted_static.len() as u16);
        buf.put(msg.encrypted_static.as_slice());
        
        buf.put_u16(msg.encrypted_timestamp.len() as u16);
        buf.put(msg.encrypted_timestamp.as_slice());
        
        buf.put(&msg.mac1[..]);
        buf.put(&msg.mac2[..]);
        
        buf.to_vec()
    }
    
    fn allocate_sender_index(&self) -> u32 {
        let mut guard = self.next_sender_index.write();
        let index = *guard;
        *guard = index.wrapping_add(1);
        index
    }
    
    pub async fn handle_packet(&self, buf: &[u8], addr: SocketAddr) -> Result<Option<Bytes>> {
        if buf.is_empty() {
            return Ok(None);
        }
        
        let msg_type = buf[0];
        
        match msg_type {
            1 => {
                info!("Received handshake initiation from {}", addr);
                self.handle_handshake_initiation(buf, addr).await
            }
            2 => {
                debug!("Received handshake response");
                self.handle_handshake_response(buf).await
            }
            4 => {
                debug!("Received transport data");
                self.handle_transport_data(buf).await
            }
            _ => {
                warn!("Unknown message type: {}", msg_type);
                Ok(None)
            }
        }
    }
    
    async fn handle_handshake_initiation(&self, buf: &[u8], addr: SocketAddr) -> Result<Option<Bytes>> {
        if buf.len() < 4 {
            anyhow::bail!("Invalid handshake initiation");
        }
        
        let sender_index = u32::from_be_bytes([buf[3], buf[4], buf[5], buf[6]]);
        
        let should_respond = {
            let peers_guard = self.peers.read();
            peers_guard.iter().any(|p| {
                p.endpoint.map(|e| e == addr).unwrap_or(false)
            })
        };
        
        if should_respond {
            let response = HandshakeResponse {
                sender_index: self.allocate_sender_index(),
                receiver_index: sender_index,
                ephemeral: [0u8; 32],
                encrypted_nothing: Vec::new(),
                mac1: [0u8; 16],
                mac2: [0u8; 16],
            };
            
            let msg = self.encode_handshake_response(&response);
            
            self.socket.send_to(&msg, addr).await?;
            info!("Handshake response sent to {}", addr);
        }
        
        Ok(None)
    }
    
    async fn handle_handshake_response(&self, _buf: &[u8]) -> Result<Option<Bytes>> {
        info!("Handshake response received, session established");
        Ok(None)
    }
    
    async fn handle_transport_data(&self, buf: &[u8]) -> Result<Option<Bytes>> {
        if buf.len() < 32 {
            anyhow::bail!("Invalid transport message");
        }
        
        let receiver_index = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let counter = u64::from_le_bytes([buf[4], buf[5], buf[6], buf[7], buf[8], buf[9], buf[10], buf[11]]);
        
        debug!("Transport data: receiver={}, counter={}", receiver_index, counter);
        
        let encrypted = buf[16..].to_vec();
        let decrypted = self.decrypt_packet(receiver_index, counter, &encrypted)?;
        
        Ok(Some(Bytes::from(decrypted)))
    }
    
    fn decrypt_packet(&self, _receiver_index: u32, _counter: u64, encrypted: &[u8]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };
        
        let sessions = self.sessions.read();
        if let Some(session) = sessions.iter().find(|s| s.remote_index == _receiver_index) {
            let key = chacha20poly1305::Key::from_slice(&session.receiving_key.key);
            let cipher = ChaCha20Poly1305::new(key);
            
            let mut nonce = [0u8; 12];
            nonce[4..].copy_from_slice(&_counter.to_le_bytes());
            
            let plaintext = cipher.decrypt(Nonce::from_slice(&nonce), encrypted)
                .map_err(|_| anyhow::anyhow!("Decryption failed"))?;
            
            return Ok(plaintext);
        }
        
        anyhow::bail!("No session found for receiver index")
    }
    
    fn encode_handshake_response(&self, msg: &HandshakeResponse) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(92);
        
        buf.put_u8(MessageType::HandshakeResponse as u8);
        buf.put_u8(0);
        buf.put_u16(40);
        buf.put_u32(msg.sender_index);
        buf.put_u32(msg.receiver_index);
        buf.put(&msg.ephemeral[..]);
        
        buf.put_u16(msg.encrypted_nothing.len() as u16);
        buf.put(msg.encrypted_nothing.as_slice());
        
        buf.put(&msg.mac1[..]);
        buf.put(&msg.mac2[..]);
        
        buf.to_vec()
    }
    
    pub async fn send_packet(&self, peer_public_key: &[u8; 32], packet: &[u8]) -> Result<()> {
        let endpoint = {
            let peers_guard = self.peers.read();
            let peer = peers_guard.iter()
                .find(|p| p.public_key == *peer_public_key)
                .with_context(|| "Peer not found")?;
            peer.endpoint
        };
        
        let endpoint = endpoint.with_context(|| "Peer has no endpoint")?;
        
        let mut buf = BytesMut::with_capacity(40 + packet.len());
        
        let session_opt = {
            let sessions = self.sessions.read();
            sessions.iter()
                .find(|s| s.peer_index == u32::from_be_bytes([peer_public_key[0], peer_public_key[1], peer_public_key[2], peer_public_key[3]]))
                .cloned()
        };
        
        if let Some(s) = session_opt {
            let encrypted_packet = self.encrypt_data(&s.sending_key.key, packet)?;
            
            buf.put_u32(s.remote_index);
            buf.put_u64(s.sending_key.counter);
            buf.put_u32(0);
            buf.put(encrypted_packet.as_slice());
        } else {
            self.initiate_handshake(peer_public_key).await?;
            return Ok(());
        }
        
        self.socket.send_to(&buf, endpoint).await?;
        
        Ok(())
    }
    
    fn encrypt_data(&self, key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce,
        };
        
        let key = chacha20poly1305::Key::from_slice(key);
        let cipher = ChaCha20Poly1305::new(key);
        
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce[4..]);
        
        let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), plaintext)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        
        Ok(ciphertext)
    }
    
    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("WireGuard tunnel listening on {}", self.socket.local_addr()?);
        
        let socket = self.socket.clone();
        let tunnel = self.clone();
        
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, addr)) => {
                        match tunnel.handle_packet(&buf[..len], addr).await {
                            Ok(Some(packet)) => {
                                debug!("Decrypted packet: {} bytes", packet.len());
                            }
                            Ok(None) => {}
                            Err(e) => {
                                error!("Error handling packet: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Socket error: {}", e);
                        break;
                    }
                }
            }
        });
        
        Ok(())
    }
}

impl Clone for WireGuardTunnel {
    fn clone(&self) -> Self {
        Self {
            socket: self.socket.clone(),
            local_private_key: self.local_private_key,
            local_public_key: self.local_public_key,
            peers: self.peers.clone(),
            sessions: self.sessions.clone(),
            next_sender_index: self.next_sender_index.clone(),
            keepalive_sender: self.keepalive_sender.clone(),
        }
    }
}

fn generate_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

fn derive_public_key(private_key: &[u8; 32]) -> [u8; 32] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(private_key);
    let result = hasher.finalize();
    let mut public = [0u8; 32];
    public.copy_from_slice(&result[..32]);
    public[0] &= 248;
    public[31] &= 127;
    public[31] |= 64;
    public
}

fn create_timestamp() -> [u8; 12] {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let mut timestamp = [0u8; 12];
    timestamp[..8].copy_from_slice(&now.to_le_bytes());
    timestamp[8..].copy_from_slice(&1u32.to_le_bytes());
    timestamp
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn generate_keypair() -> Result<([u8; 32], [u8; 32])> {
    let private = generate_key();
    let public = derive_public_key(&private);
    Ok((private, public))
}
