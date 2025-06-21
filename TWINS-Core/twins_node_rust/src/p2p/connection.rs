// TWINS-Core/twins_node_rust/src/p2p/connection.rs
// This file will contain logic for establishing and managing P2P connections.

use crate::p2p::messages::{
    MessageHeader, VersionMessage, Encodable, Decodable,
    PingMessage, PongMessage,
    MAINNET_MAGIC, CMD_VERSION, CMD_VERACK, CMD_PING, CMD_PONG,
    NODE_NETWORK, MIN_PEER_PROTO_VERSION,
};
use crate::p2p::seeds::MAINNET_SEED_NODES;
use crate::p2p::peer::Peer;
use crate::p2p::peer_manager::{PeerManager, handle_peer_session};
use crate::blockchain::chain_state::ChainState;
use crate::storage::BlockStorage;
use crate::spork_manager::SporkManager;
use crate::masternode_manager::MasternodeManager;
use crate::governance_manager::GovernanceManager;
use crate::instant_send_manager::InstantSendManager;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use std::io::{Cursor, Error as IoError, ErrorKind as IoErrorKind};
use std::sync::Arc;
use log::{info, warn, error, debug};
use sha2::{Sha256, Digest};
use rand;

// Made public for use by peer session tasks
pub fn calculate_checksum(payload: &[u8]) -> [u8; 4] {
    let hash1 = Sha256::digest(payload);
    let hash2 = Sha256::digest(&hash1);
    let mut checksum = [0u8; 4];
    checksum.copy_from_slice(&hash2[0..4]);
    checksum
}

pub async fn try_connect_to_seeds(
    peer_manager: Arc<PeerManager>,
    chain_state: Arc<ChainState>,
    storage: Arc<dyn BlockStorage>,
    spork_manager: Arc<SporkManager>,
    masternode_manager: Arc<MasternodeManager>,
    governance_manager: Arc<GovernanceManager>,
    instant_send_manager: Arc<InstantSendManager>,
    parallel_sync_manager: Option<Arc<crate::p2p::parallel_sync_manager::ParallelSyncManager>>,
    hybrid_sync_manager: Option<Arc<crate::p2p::hybrid_sync::HybridSyncManager>>,
    our_start_height: i32, 
    our_services: u64
) {
    info!("Attempting to connect to seed nodes...");
    let mut connect_tasks = vec![];
    for seed_addr_str in MAINNET_SEED_NODES {
        match seed_addr_str.parse::<SocketAddr>() {
            Ok(socket_addr) => {
                let pm_clone = Arc::clone(&peer_manager);
                let cs_clone = Arc::clone(&chain_state);
                let storage_clone = Arc::clone(&storage);
                let spork_manager_clone = Arc::clone(&spork_manager);
                let mn_manager_clone = Arc::clone(&masternode_manager);
                let gov_manager_clone = Arc::clone(&governance_manager);
                let ix_manager_clone = Arc::clone(&instant_send_manager);
                let psm_clone = parallel_sync_manager.clone();
                let hsm_clone = hybrid_sync_manager.clone();
                connect_tasks.push(tokio::spawn(async move {
                    debug!("Attempting to connect to seed: {}", socket_addr);
                    match TcpStream::connect(socket_addr).await {
                        Ok(stream) => { 
                            info!("Successfully connected to seed: {}", socket_addr);
                            match perform_full_interaction(stream, socket_addr, our_start_height, our_services).await {
                                Ok((peer_version_info, connected_stream)) => {
                                    info!("Full interaction with {} successful. Peer version: {}, User Agent: {}, Height: {}", 
                                        socket_addr, peer_version_info.version, peer_version_info.user_agent, peer_version_info.start_height);
                                    
                                    let mut new_peer_obj = Peer::new(socket_addr, connected_stream);
                                    new_peer_obj.set_version_data(&peer_version_info);
                                    new_peer_obj.handshake_completed = true;
                                    
                                    let peer_arc = pm_clone.add_peer_to_map(new_peer_obj);
                                    
                                    // Register peer with parallel sync manager
                                    if let Some(ref psm) = psm_clone {
                                        psm.add_peer(socket_addr).await;
                                    }
                                    
                                    // Register peer with hybrid sync manager
                                    if let Some(ref hsm) = hsm_clone {
                                        hsm.register_peer(socket_addr, &peer_version_info.user_agent, peer_version_info.version).await;
                                    }
                                    
                                    tokio::spawn(async move {
                                        handle_peer_session(peer_arc, pm_clone, cs_clone, storage_clone, spork_manager_clone, mn_manager_clone, gov_manager_clone, ix_manager_clone, psm_clone.clone(), hsm_clone.clone()).await;
                                    });
                                }
                                Err(e) => {
                                    error!("Initial interaction with {} failed: {}", socket_addr, e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to connect to seed {}: {}", socket_addr, e);
                        }
                    }
                }));
            }
            Err(e) => {
                error!("Failed to parse seed address {}: {}", seed_addr_str, e);
            }
        }
    }
    for task in connect_tasks {
        let _ = task.await;
    }
    info!("Finished initial connection attempts to seeds. PeerManager has {} active connections.", peer_manager.get_peer_count());
    
    // Keep the P2P task alive with automatic reconnection
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        let peer_count = peer_manager.get_peer_count();
        info!("P2P status check: {} active peer connections", peer_count);
        
        // Implement automatic reconnection when peer count is low
        if peer_count < 3 {
            warn!("Low peer count ({}), attempting to reconnect to seeds...", peer_count);
            
            // Retry connections to seed nodes
            let mut reconnect_tasks = vec![];
            for seed_addr_str in MAINNET_SEED_NODES {
                if let Ok(socket_addr) = seed_addr_str.parse::<SocketAddr>() {
                    // Skip if we already have this peer
                    let peers_map = peer_manager.peers.lock().unwrap();
                    if peers_map.contains_key(&socket_addr) {
                        continue;
                    }
                    drop(peers_map);
                    
                    let pm_clone = Arc::clone(&peer_manager);
                    let cs_clone = Arc::clone(&chain_state);
                    let storage_clone = Arc::clone(&storage);
                    let spork_manager_clone = Arc::clone(&spork_manager);
                    let mn_manager_clone = Arc::clone(&masternode_manager);
                    let gov_manager_clone = Arc::clone(&governance_manager);
                    let ix_manager_clone = Arc::clone(&instant_send_manager);
                    let psm_clone = parallel_sync_manager.clone();
                    let hsm_clone = hybrid_sync_manager.clone();
                    
                    reconnect_tasks.push(tokio::spawn(async move {
                        debug!("Attempting to reconnect to seed: {}", socket_addr);
                        match TcpStream::connect(socket_addr).await {
                            Ok(stream) => { 
                                info!("Successfully reconnected to seed: {}", socket_addr);
                                match perform_full_interaction(stream, socket_addr, our_start_height, our_services).await {
                                    Ok((peer_version_info, connected_stream)) => {
                                        info!("Reconnection handshake with {} successful", socket_addr);
                                        
                                        let mut new_peer_obj = Peer::new(socket_addr, connected_stream);
                                        new_peer_obj.set_version_data(&peer_version_info);
                                        new_peer_obj.handshake_completed = true;
                                        
                                        let peer_arc = pm_clone.add_peer_to_map(new_peer_obj);
                                        
                                        // Register peer with parallel sync manager
                                        if let Some(ref psm) = psm_clone {
                                            psm.add_peer(socket_addr).await;
                                        }
                                        
                                        // Register peer with hybrid sync manager
                                        if let Some(ref hsm) = hsm_clone {
                                            hsm.register_peer(socket_addr, &peer_version_info.user_agent, peer_version_info.version).await;
                                        }
                                        
                                        tokio::spawn(async move {
                                            handle_peer_session(peer_arc, pm_clone, cs_clone, storage_clone, spork_manager_clone, mn_manager_clone, gov_manager_clone, ix_manager_clone, psm_clone.clone(), hsm_clone.clone()).await;
                                        });
                                    }
                                    Err(e) => {
                                        warn!("Reconnection handshake with {} failed: {}", socket_addr, e);
                                    }
                                }
                            }
                            Err(e) => {
                                debug!("Failed to reconnect to seed {}: {}", socket_addr, e);
                            }
                        }
                    }));
                }
            }
            
            // Wait for reconnection attempts (with timeout)
            let reconnect_timeout = tokio::time::Duration::from_secs(10);
            for task in reconnect_tasks {
                let _ = tokio::time::timeout(reconnect_timeout, task).await;
            }
            
            let new_peer_count = peer_manager.get_peer_count();
            info!("Reconnection attempt completed. Peer count: {} -> {}", peer_count, new_peer_count);
        }
    }
}

async fn perform_full_interaction(
    mut stream: TcpStream, 
    peer_addr: SocketAddr,
    our_start_height: i32,
    our_services: u64,
) -> Result<(VersionMessage, TcpStream), IoError> { // Returns VersionMessage and owned Stream
    info!("Performing handshake and ping/pong with {}", peer_addr);
    let our_nonce: u64 = rand::random();

    // 1. Send our VersionMessage
    let version_msg_payload = VersionMessage::default_for_peer(
        peer_addr.ip(),
        peer_addr.port(),
        our_start_height,
        our_services,
        our_nonce,
    );
    let mut payload_bytes_vec = Vec::new();
    version_msg_payload.consensus_encode(&mut Cursor::new(&mut payload_bytes_vec))?;
    let checksum_ours = calculate_checksum(&payload_bytes_vec);
    let header_ours = MessageHeader::new(*CMD_VERSION, payload_bytes_vec.len() as u32, checksum_ours);
    send_message(&mut stream, header_ours, &payload_bytes_vec).await?;
    info!("Sent Version message to {}", peer_addr);

    // 2. Receive peer's VersionMessage
    debug!("Waiting for Version/Verack from {}", peer_addr);
    let (peer_version_header, peer_version_payload_bytes) = read_network_message(&mut stream).await?;

    if &peer_version_header.command == CMD_VERSION {
        let calculated_checksum = calculate_checksum(&peer_version_payload_bytes);
        if calculated_checksum != peer_version_header.checksum {
            return Err(IoError::new(IoErrorKind::InvalidData, 
                format!("Version checksum mismatch. Expected {:?}, got {:?}", peer_version_header.checksum, calculated_checksum)
            ));
        }
        let peer_version_msg = VersionMessage::consensus_decode(&mut Cursor::new(&peer_version_payload_bytes))?;
        info!("Received Version message from {}: V={}, Services={}, UserAgent=\"{}\", Height={}, Nonce={}", 
                peer_addr, peer_version_msg.version, peer_version_msg.services, peer_version_msg.user_agent, peer_version_msg.start_height, peer_version_msg.nonce);
        
        if peer_version_msg.nonce == our_nonce {
            return Err(IoError::new(IoErrorKind::Other, "Connected to self (same nonce)"));
        }
        if peer_version_msg.version < MIN_PEER_PROTO_VERSION {
            error!("Peer {} has outdated protocol version {}. Minimum required: {}. Disconnecting.", 
                peer_addr, peer_version_msg.version, MIN_PEER_PROTO_VERSION);
            return Err(IoError::new(IoErrorKind::Other, "Peer protocol version too old"));
        }
        if (peer_version_msg.services & NODE_NETWORK) == 0 {
            error!("Peer {} does not offer NODE_NETWORK service. Disconnecting.", peer_addr);
            return Err(IoError::new(IoErrorKind::Other, "Peer does not offer NODE_NETWORK"));
        }

        // 3. Send our VerackMessage
        send_empty_payload_message(&mut stream, *CMD_VERACK).await?;
        info!("Sent Verack message to {}", peer_addr);

        // 4. Receive peer's VerackMessage
        debug!("Waiting for Verack from {}", peer_addr);
        let (peer_verack_header, peer_verack_payload_bytes) = read_network_message(&mut stream).await?;

        if &peer_verack_header.command == CMD_VERACK {
            if peer_verack_header.length != 0 {
                return Err(IoError::new(IoErrorKind::InvalidData, "Verack message has non-zero payload length"));
            }
            let calculated_verack_checksum = calculate_checksum(&peer_verack_payload_bytes);
            if calculated_verack_checksum != peer_verack_header.checksum {
                 return Err(IoError::new(IoErrorKind::InvalidData, 
                    format!("Verack checksum mismatch. Expected {:?}, got {:?}", peer_verack_header.checksum, calculated_verack_checksum)
                ));
            }
            info!("Received Verack message from {}. Handshake complete.", peer_addr);

            // 5. Send Ping
            let ping_nonce: u64 = rand::random();
            let ping_payload = PingMessage::new(ping_nonce);
            let mut ping_payload_bytes = Vec::new();
            ping_payload.consensus_encode(&mut Cursor::new(&mut ping_payload_bytes))?;
            let ping_checksum = calculate_checksum(&ping_payload_bytes);
            let ping_header = MessageHeader::new(*CMD_PING, ping_payload_bytes.len() as u32, ping_checksum);
            send_message(&mut stream, ping_header, &ping_payload_bytes).await?;
            info!("Sent Ping (nonce: {}) to {}", ping_nonce, peer_addr);

            // 6. Handle next message (could be PONG or PING)
            debug!("Waiting for Pong or handling Ping from {}", peer_addr);
            let (next_header, next_payload_bytes) = read_network_message(&mut stream).await?;
            if &next_header.command == CMD_PONG {
                let pong_calculated_checksum = calculate_checksum(&next_payload_bytes);
                if pong_calculated_checksum != next_header.checksum {
                    return Err(IoError::new(IoErrorKind::InvalidData, "Pong checksum mismatch"));
                }
                let pong_msg = PongMessage::consensus_decode(&mut Cursor::new(&next_payload_bytes))?;
                if pong_msg.nonce == ping_nonce {
                    info!("Received valid Pong (nonce: {}) from {}", pong_msg.nonce, peer_addr);
                    Ok((peer_version_msg, stream)) // Return VersionMessage and owned Stream
                } else {
                    Err(IoError::new(IoErrorKind::InvalidData, 
                        format!("Pong nonce mismatch. Expected {}, got {}", ping_nonce, pong_msg.nonce)))
                }
            } else if &next_header.command == CMD_PING {
                // Peer sent us a PING, respond with PONG first
                let ping_calculated_checksum = calculate_checksum(&next_payload_bytes);
                if ping_calculated_checksum != next_header.checksum {
                    return Err(IoError::new(IoErrorKind::InvalidData, "Ping checksum mismatch"));
                }
                let ping_msg = PingMessage::consensus_decode(&mut Cursor::new(&next_payload_bytes))?;
                info!("Received Ping (nonce: {}) from {}, responding with Pong", ping_msg.nonce, peer_addr);
                
                // Send PONG response
                let pong_payload = PongMessage::new(ping_msg.nonce);
                let mut pong_payload_bytes = Vec::new();
                pong_payload.consensus_encode(&mut Cursor::new(&mut pong_payload_bytes))?;
                let pong_checksum = calculate_checksum(&pong_payload_bytes);
                let pong_header = MessageHeader::new(*CMD_PONG, pong_payload_bytes.len() as u32, pong_checksum);
                send_message(&mut stream, pong_header, &pong_payload_bytes).await?;
                info!("Sent Pong (nonce: {}) to {}", ping_msg.nonce, peer_addr);
                
                // Now wait for our PONG response
                debug!("Now waiting for our Pong from {}", peer_addr);
                let (pong_header, pong_payload_bytes) = read_network_message(&mut stream).await?;
                if &pong_header.command == CMD_PONG {
                    let pong_calculated_checksum = calculate_checksum(&pong_payload_bytes);
                    if pong_calculated_checksum != pong_header.checksum {
                        return Err(IoError::new(IoErrorKind::InvalidData, "Pong checksum mismatch"));
                    }
                    let pong_msg = PongMessage::consensus_decode(&mut Cursor::new(&pong_payload_bytes))?;
                    if pong_msg.nonce == ping_nonce {
                        info!("Received valid Pong (nonce: {}) from {} after ping exchange", pong_msg.nonce, peer_addr);
                        Ok((peer_version_msg, stream)) // Return VersionMessage and owned Stream
                    } else {
                        Err(IoError::new(IoErrorKind::InvalidData, 
                            format!("Pong nonce mismatch. Expected {}, got {}", ping_nonce, pong_msg.nonce)))
                    }
                } else {
                    Err(IoError::new(IoErrorKind::InvalidData, 
                        format!("Expected PONG after ping exchange, got {:?}", String::from_utf8_lossy(&pong_header.command))))
                }
            } else {
                Err(IoError::new(IoErrorKind::InvalidData, 
                    format!("Expected PONG or PING, got {:?}", String::from_utf8_lossy(&next_header.command))))
            }
        } else {
            Err(IoError::new(IoErrorKind::InvalidData, format!("Expected VERACK after our Verack, got {:?}", String::from_utf8_lossy(&peer_verack_header.command))))
        }

    } else if &peer_version_header.command == CMD_VERACK {
        warn!("Peer {} sent VERACK immediately after our Version. Handshake might be incomplete or non-standard.", peer_addr);
        Err(IoError::new(IoErrorKind::Other, "Peer sent VERACK too early or instead of Version"))
    } else {
         Err(IoError::new(IoErrorKind::InvalidData, format!("Expected VERSION or VERACK, got command: {:?}", String::from_utf8_lossy(&peer_version_header.command))))
    }
}

// Made public for use by peer session tasks
pub async fn send_message(stream: &mut TcpStream, header: MessageHeader, payload: &[u8]) -> Result<(), IoError> {
    let mut header_bytes = Vec::new();
    header.consensus_encode(&mut Cursor::new(&mut header_bytes))?;
    stream.write_all(&header_bytes).await?;
    if !payload.is_empty() {
        stream.write_all(payload).await?;
    }
    Ok(())
}

// Made public for use by peer session tasks
pub async fn send_empty_payload_message(stream: &mut TcpStream, command: [u8; 12]) -> Result<(), IoError> {
    let checksum = calculate_checksum(&[]);
    let header = MessageHeader::new(command, 0, checksum);
    send_message(stream, header, &[]).await
}

// Made public for use by peer session tasks
pub async fn read_network_message(stream: &mut TcpStream) -> Result<(MessageHeader, Vec<u8>), IoError> {
    let mut header_buf = [0u8; MessageHeader::SIZE];
    stream.read_exact(&mut header_buf).await?;
    let header = MessageHeader::consensus_decode(&mut Cursor::new(&header_buf))?;
    debug!("Received header: {:?}", header);

    if header.magic != MAINNET_MAGIC {
        return Err(IoError::new(IoErrorKind::InvalidData, 
            format!("Invalid magic bytes: {:?}. Expected: {:?}", header.magic, MAINNET_MAGIC)
        ));
    }

    if header.length > 2 * 1024 * 1024 { 
        return Err(IoError::new(IoErrorKind::InvalidData, 
            format!("Payload length {} exceeds reasonable limit (e.g. > 2MB)", header.length)
        ));
    }

    let mut payload_buf = vec![0u8; header.length as usize];
    if header.length > 0 {
        stream.read_exact(&mut payload_buf).await?;
    }
    
    debug!("Received payload of length: {}", payload_buf.len());
    Ok((header, payload_buf))
}

// Removed handle_established_peer, its functionality will be part of a dedicated peer task system later.

// TODO:
// - Import necessary modules (tokio::net, super::messages, super::seeds, std::net::SocketAddr, etc.)
// - Define a function to attempt connections to seed nodes.
// - Implement the P2P handshake logic (version/verack exchange). 