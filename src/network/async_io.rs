use std::sync::Arc;
use std::net::{SocketAddr, IpAddr};
use tokio::net::{TcpStream, UdpSocket};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, RwLock};
use anyhow::{Result, Context};
use bytes::{Bytes, BytesMut, BufMut};
use tracing::{info, debug, warn, error};
use parking_lot::RwLock as PLRwLock;

pub struct AsyncTcpStream {
    stream: TcpStream,
    read_buffer: BytesMut,
    write_buffer: BytesMut,
}

impl AsyncTcpStream {
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let stream = TcpStream::connect(addr).await
            .with_context(|| format!("Failed to connect to {}", addr))?;
        
        Ok(Self {
            stream,
            read_buffer: BytesMut::with_capacity(64 * 1024),
            write_buffer: BytesMut::with_capacity(64 * 1024),
        })
    }
    
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.read_buffer.is_empty() {
            self.read_buffer.reserve(8192);
            let n = self.stream.read_buf(&mut self.read_buffer).await?;
            if n == 0 {
                return Ok(0);
            }
        }
        
        let to_read = std::cmp::min(buf.len(), self.read_buffer.len());
        buf[..to_read].copy_from_slice(&self.read_buffer[..to_read]);
        self.read_buffer.advance(to_read);
        
        Ok(to_read)
    }
    
    pub async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.write_buffer.put(data);
        
        while !self.write_buffer.is_empty() {
            let n = self.stream.write_buf(&mut self.write_buffer).await?;
            if n == 0 {
                anyhow::bail!("Connection closed");
            }
        }
        
        Ok(())
    }
    
    pub async fn flush(&mut self) -> Result<()> {
        self.stream.flush().await?;
        Ok(())
    }
}

pub struct AsyncUdpSocket {
    socket: Arc<UdpSocket>,
    local_addr: SocketAddr,
}

impl AsyncUdpSocket {
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(addr).await
            .with_context(|| format!("Failed to bind to {}", addr))?;
        
        let local_addr = socket.local_addr()?;
        
        Ok(Self {
            socket: Arc::new(socket),
            local_addr,
        })
    }
    
    pub async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        let n = self.socket.send_to(buf, addr).await?;
        Ok(n)
    }
    
    pub async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr)> {
        let (n, addr) = self.socket.recv_from(buf).await?;
        Ok((n, addr))
    }
    
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

pub struct PacketRouter {
    socket: Arc<UdpSocket>,
    connections: Arc<PLRwLock<std::collections::HashMap<SocketAddr, ConnectionState>>>,
    event_sender: mpsc::UnboundedSender<RouterEvent>,
}

#[derive(Clone)]
struct ConnectionState {
    last_seen: std::time::Instant,
    packets_sent: u64,
    packets_received: u64,
}

#[derive(Debug, Clone)]
pub enum RouterEvent {
    PacketReceived { from: SocketAddr, data: Bytes },
    PeerConnected(SocketAddr),
    PeerDisconnected(SocketAddr),
}

impl PacketRouter {
    pub fn new(socket: Arc<UdpSocket>) -> Self {
        let (tx, _) = mpsc::unbounded_channel();
        
        Self {
            socket,
            connections: Arc::new(PLRwLock::new(std::collections::HashMap::new())),
            event_sender: tx,
        }
    }
    
    pub async fn start_router(mut self) {
        let socket = self.socket.clone();
        let connections = self.connections.clone();
        let sender = self.event_sender.clone();
        
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65535];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, addr)) => {
                        {
                            let mut conns = connections.write();
                            if let Some(state) = conns.get_mut(&addr) {
                                state.last_seen = std::time::Instant::now();
                                state.packets_received += 1;
                            } else {
                                conns.insert(addr, ConnectionState {
                                    last_seen: std::time::Instant::now(),
                                    packets_sent: 0,
                                    packets_received: 1,
                                });
                                
                                if let Err(e) = sender.send(RouterEvent::PeerConnected(addr)) {
                                    warn!("Failed to send peer connected event: {}", e);
                                }
                            }
                        }
                        
                        let data = Bytes::copy_from_slice(&buf[..len]);
                        if let Err(e) = sender.send(RouterEvent::PacketReceived { from: addr, data }) {
                            warn!("Failed to send packet event: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Socket error: {}", e);
                        break;
                    }
                }
            }
        });
    }
    
    pub async fn send_packet(&self, data: &[u8], addr: SocketAddr) -> Result<usize> {
        {
            let mut conns = self.connections.write();
            if let Some(state) = conns.get_mut(&addr) {
                state.last_seen = std::time::Instant::now();
                state.packets_sent += 1;
            }
        }
        
        self.socket.send_to(data, addr).await
    }
    
    pub fn take_event_receiver(&self) -> mpsc::UnboundedReceiver<RouterEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        
        let old_sender = std::mem::replace(
            unsafe { &mut *(self.event_sender.as_ptr() as *mut mpsc::UnboundedSender<RouterEvent>) },
            tx
        );
        
        drop(old_sender);
        rx
    }
    
    pub fn get_connection_count(&self) -> usize {
        self.connections.read().len()
    }
}

pub struct ConnectionPool {
    max_connections: usize,
    connections: Arc<PLRwLock<Vec<ConnectionHandle>>>,
}

struct ConnectionHandle {
    peer_id: String,
    socket: Arc<UdpSocket>,
    endpoint: Option<SocketAddr>,
    created_at: std::time::Instant,
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self {
            max_connections,
            connections: Arc::new(PLRwLock::new(Vec::new())),
        }
    }
    
    pub fn add_connection(&self, peer_id: String, socket: Arc<UdpSocket>, endpoint: SocketAddr) -> Result<()> {
        let mut conns = self.connections.write();
        
        if conns.len() >= self.max_connections {
            conns.retain(|c| {
                c.created_at.elapsed() < std::time::Duration::from_secs(300)
            });
            
            if conns.len() >= self.max_connections {
                anyhow::bail!("Connection pool full");
            }
        }
        
        conns.push(ConnectionHandle {
            peer_id,
            socket,
            endpoint: Some(endpoint),
            created_at: std::time::Instant::now(),
        });
        
        Ok(())
    }
    
    pub fn remove_connection(&self, peer_id: &str) -> Result<()> {
        let mut conns = self.connections.write();
        conns.retain(|c| c.peer_id != peer_id);
        Ok(())
    }
    
    pub fn get_connection(&self, peer_id: &str) -> Option<ConnectionHandle> {
        let conns = self.connections.read();
        conns.iter().find(|c| c.peer_id == peer_id).cloned()
    }
}
