// TWINS-Core/twins_node_rust/src/p2p/peer_manager.rs
// This file will define the PeerManager struct for managing active peer connections.

use crate::p2p::peer::Peer;
use crate::p2p::messages::{
    PingMessage, PongMessage, MessageHeader, CMD_PING, CMD_PONG, Decodable, Encodable,
    GetHeadersMessage, GetBlocksMessage, HeadersMessage, CMD_GETHEADERS, CMD_GETBLOCKS, CMD_HEADERS, // Added GetHeaders/GetBlocks/Headers
    PROTOCOL_VERSION,
    VersionMessage,
    MAX_HEADERS_PER_MSG,
    GetDataMessage, InventoryVector, InventoryType, CMD_GETDATA, // Added GetData related items
    BlockMessage, CMD_BLOCK, // Added BlockMessage & CMD_BLOCK
    CMD_INV, // Added CMD_INV
    MasternodeBroadcast, CMD_MNB, // Added for Masternode Broadcast
    SporkMessage, CMD_SPORK, // Added for Spork messages
    BudgetProposalBroadcast, CMD_MPROP, // Added for Budget Proposal Broadcast
    BudgetVote, CMD_MVOTE,             // Added for Budget Vote
    FinalizedBudgetBroadcast, CMD_FBS, // Added for Finalized Budget Broadcast
    FinalizedBudgetVote, CMD_FBVOTE,    // Added for Finalized Budget Vote
    TxLockRequest, CMD_IX,            // Added for InstantSend Tx Lock Request
    ConsensusVote, CMD_TXLVOTE,        // Added for InstantSend Tx Lock Vote
    SscMessage, // Already imported for SscMessage context, explicit here too
    DstxMessage, CMD_DSTX,             // Added for Dstx message
    MasternodeWinner, CMD_MNW,          // Added for Masternode Winner
    MasternodePing, CMD_MNP, // Explicitly add CMD_MNP if MasternodePing is used directly
    CMD_SSC,  // Explicitly add CMD_SSC if SscMessage is used directly
    CMD_GETSPORKS, // Added for GetSporks
    DsegMessage, CMD_DSEG, // Added for DSEG message
    MnGetMessage, CMD_MNGET, // Added for MNGET message
    MnvsMessage, CMD_MNVS,    // Added for MNVS message
    AddrMessage, CMD_ADDR, // Added for ADDR message
    CMD_GETADDR, // Added for GETADDR command
    RejectMessage, CMD_REJECT, // Added for REJECT message
    CMD_MEMPOOL, // Added for MEMPOOL command
    CMD_NOTFOUND, // Added for NOTFOUND command
    CMD_QFC // Placeholder for qfcommit
};
use crate::p2p::connection::{read_network_message, send_message, calculate_checksum, send_empty_payload_message}; // send_empty_payload_message might not be needed here
use crate::blockchain::chain_state::ChainState; // Added ChainState import
use crate::storage::BlockStorage; // Added storage import
use crate::spork_manager::SporkManager; // Added SporkManager import
use crate::masternode_manager::MasternodeManager; // Added MasternodeManager import
use crate::governance_manager::GovernanceManager; // Added GovernanceManager import
use crate::instant_send_manager::InstantSendManager; // Added InstantSendManager import
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex as StdMutex}; // Using StdMutex for PeerManager's map for now
use tokio::sync::Mutex as TokioMutex; // Using TokioMutex for individual Peer objects
use tokio::net::TcpStream;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Cursor}; // Added Cursor
use log::{info, warn, error, debug};
use std::time::Duration; // Added for ping interval
use tokio::time; // Added for tokio::time::interval
use tokio::io::AsyncWriteExt; // Added this import
use hex; // Added for hex::decode_to_slice
use std::collections::HashSet; // Added for session_requested_block_hashes

// TODO: Consider using RwLock for better concurrency if reads are much more frequent than writes.

#[derive(Debug)]
pub struct PeerManager {
    // Wrap the HashMap in a standard Mutex for synchronous access to the map itself.
    // Individual Peer objects will be wrapped in TokioMutex for async operations within their tasks.
    pub peers: Arc<StdMutex<HashMap<SocketAddr, Arc<TokioMutex<Peer>>>>>, 
}

impl PeerManager {
    pub fn new() -> Self {
        PeerManager {
            peers: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    // Modified to just add the peer and return the Arc for the caller to spawn tasks if needed.
    // The caller (try_connect_to_seeds) will now handle spawning handle_peer_session.
    pub fn add_peer_to_map(&self, peer_obj: Peer) -> Arc<TokioMutex<Peer>> {
        let peer_addr = peer_obj.address;
        let arc_peer = Arc::new(TokioMutex::new(peer_obj));
        {
            let mut peers_map = self.peers.lock().unwrap(); 
            info!("Adding peer to PeerManager map: {}", peer_addr);
            peers_map.insert(peer_addr, Arc::clone(&arc_peer));
        }
        arc_peer
    }

    pub fn remove_peer(&self, addr: &SocketAddr) {
        let mut peers_map = self.peers.lock().unwrap();
        if peers_map.remove(addr).is_some() {
            info!("Removed peer from PeerManager: {}", addr);
        } else {
            warn!("Attempted to remove non-existent peer: {}", addr);
        }
    }

    pub fn get_peer_count(&self) -> usize {
        self.peers.lock().unwrap().len()
    }

    // Get the highest block height reported by any connected peer
    pub async fn get_best_peer_height(&self) -> Option<u32> {
        let peers_map = self.peers.lock().unwrap();
        let mut best_height = None;
        
        for peer_arc in peers_map.values() {
            if let Ok(peer_guard) = peer_arc.try_lock() {
                if let Some(start_height) = peer_guard.start_height {
                    let height = start_height as u32;
                    match best_height {
                        None => best_height = Some(height),
                        Some(current_best) => {
                            if height > current_best {
                                best_height = Some(height);
                            }
                        }
                    }
                }
            }
        }
        
        best_height
    }

    // TODO: Add methods for:
    // - Getting a specific peer
    // - Iterating over peers (e.g., for broadcasting messages)
    // - Periodically pinging peers / checking for dead connections
    // - Handling peer disconnections initiated by the peer or by us
}

// Constants for PING and SYNC - Optimized for faster syncing
const PING_INTERVAL_SECONDS: u64 = 45; // Send a ping every 45 seconds
const MAX_BLOCKS_TO_REQUEST_PER_GETDATA: usize = 500; // Increased to 500 for much faster sync
const SYNC_STATUS_INTERVAL_SECONDS: u64 = 10; // Reduced from 30 to 10 seconds for faster stall detection
const MAX_PENDING_BLOCKS: usize = 2000; // Maximum blocks to queue for download (increased for faster sync)

async fn send_getheaders_message(stream: &mut TcpStream, locator_hashes: Vec<[u8; 32]>, hash_stop: [u8; 32]) -> Result<(), IoError> {
    let getheaders_payload = GetHeadersMessage {
        version: PROTOCOL_VERSION,
        block_locator_hashes: locator_hashes,
        hash_stop,
    };
    let mut getheaders_payload_bytes = Vec::new();
    getheaders_payload.consensus_encode(&mut std::io::Cursor::new(&mut getheaders_payload_bytes))?;
    let checksum = calculate_checksum(&getheaders_payload_bytes);
    let header = MessageHeader::new(*CMD_GETHEADERS, getheaders_payload_bytes.len() as u32, checksum);
    send_message(stream, header, &getheaders_payload_bytes).await
}

async fn send_getblocks_message(stream: &mut TcpStream, locator_hashes: Vec<[u8; 32]>, hash_stop: [u8; 32]) -> Result<(), IoError> {
    let getblocks_payload = GetBlocksMessage {
        version: PROTOCOL_VERSION,
        block_locator_hashes: locator_hashes,
        hash_stop,
    };
    let mut getblocks_payload_bytes = Vec::new();
    getblocks_payload.consensus_encode(&mut std::io::Cursor::new(&mut getblocks_payload_bytes))?;
    let checksum = calculate_checksum(&getblocks_payload_bytes);
    let header = MessageHeader::new(*CMD_GETBLOCKS, getblocks_payload_bytes.len() as u32, checksum);
    info!("Sending GETBLOCKS with {} locator hashes", getblocks_payload.block_locator_hashes.len());
    send_message(stream, header, &getblocks_payload_bytes).await
}

async fn send_getsporks_message(stream: &mut TcpStream) -> Result<(), IoError> {
    debug!("Sending GETSPORKS message");
    send_empty_payload_message(stream, *CMD_GETSPORKS).await
}

async fn send_mnget_message(stream: &mut TcpStream, count: i32) -> Result<(), IoError> {
    debug!("Sending MNGET message with count: {}", count);
    let mnget_payload_data = MnGetMessage { count };
    let mut mnget_payload_bytes = Vec::new();
    mnget_payload_data.consensus_encode(&mut std::io::Cursor::new(&mut mnget_payload_bytes))?;
    let checksum = calculate_checksum(&mnget_payload_bytes);
    let header = MessageHeader::new(*CMD_MNGET, mnget_payload_bytes.len() as u32, checksum);
    send_message(stream, header, &mnget_payload_bytes).await
}

async fn send_getaddr_message(stream: &mut TcpStream) -> Result<(), IoError> {
    debug!("Sending GETADDR message");
    send_empty_payload_message(stream, *CMD_GETADDR).await
}

async fn send_mempool_message(stream: &mut TcpStream) -> Result<(), IoError> {
    debug!("Sending MEMPOOL message");
    send_empty_payload_message(stream, *CMD_MEMPOOL).await
}

// This function will now be called by the task spawner in connection.rs
pub async fn handle_peer_session(
    peer_arc: Arc<TokioMutex<Peer>>,
    peer_manager_arc: Arc<PeerManager>,
    chain_state: Arc<ChainState>,
    storage: Arc<dyn BlockStorage>,
    spork_manager: Arc<SporkManager>,
    masternode_manager: Arc<MasternodeManager>,
    governance_manager: Arc<GovernanceManager>,
    instant_send_manager: Arc<InstantSendManager>, // Added instant_send_manager argument
    parallel_sync_manager: Option<Arc<crate::p2p::parallel_sync_manager::ParallelSyncManager>>, // Added parallel sync manager
    hybrid_sync_manager: Option<Arc<crate::p2p::hybrid_sync::HybridSyncManager>>, // Added hybrid sync manager
) {
    let peer_addr: SocketAddr;
    let stream_opt: Option<TcpStream>;
    let version_info_opt: Option<VersionMessage>;

    {
        let mut peer_guard = peer_arc.lock().await;
        peer_addr = peer_guard.address;
        stream_opt = peer_guard.take_stream();
        version_info_opt = peer_guard.version_info.clone();
    }

    if let Some(mut stream) = stream_opt {
        info!("Starting message handling loop for peer: {}", peer_addr);
        let mut ping_interval = time::interval(Duration::from_secs(PING_INTERVAL_SECONDS));
        
        let mut initial_requests_successfully_sent = false;

        let mut pending_block_hashes_to_download: Vec<[u8; 32]> = Vec::new();
        let mut session_requested_block_hashes: HashSet<[u8; 32]> = HashSet::new();
        let mut last_request_time = std::time::Instant::now();
        const REQUEST_TIMEOUT_SECONDS: u64 = 30; // Timeout for block requests

        if version_info_opt.is_some() {
            let mut ok = true;
            if ok && send_getblocks_message(&mut stream, chain_state.get_block_locator(), [0u8; 32]).await.is_err() {
                error!("Failed to send initial GETBLOCKS to {}. Terminating session.", peer_addr); ok = false;
            } else if ok { info!("Sent initial GETBLOCKS to {}", peer_addr); }
            
            if ok && send_getsporks_message(&mut stream).await.is_err() {
                error!("Failed to send initial GETSPORKS to {}.", peer_addr); ok = false;
            } else if ok { info!("Sent initial GETSPORKS to {}", peer_addr); }

            if ok && masternode_manager.request_masternode_list(&mut stream).await.is_err() {
                error!("Failed to request masternode list via MasternodeManager from {}.", peer_addr); ok = false;
            } else if ok { info!("Requested masternode list via MasternodeManager from {}", peer_addr); }

            if ok && send_mnget_message(&mut stream, 0).await.is_err() { 
                error!("Failed to send initial MNGET to {}.", peer_addr); ok = false;
            } else if ok { info!("Sent initial MNGET to {}", peer_addr); }

            if ok && governance_manager.request_budget_sync(&mut stream).await.is_err() { 
                error!("Failed to request budget sync via GovernanceManager for {}.", peer_addr);
                ok = false; 
            } else if ok { 
                info!("Requested budget sync via GovernanceManager for {}.", peer_addr);
            }
            
            if ok && send_getaddr_message(&mut stream).await.is_err() {
                error!("Failed to send GETADDR to {}.", peer_addr); ok = false;
            } else if ok { info!("Sent GETADDR to {}", peer_addr); }

            if ok && send_mempool_message(&mut stream).await.is_err() {
                error!("Failed to send MEMPOOL to {}.", peer_addr); 
                // ok = false; // Mempool request failure might not be fatal for session
            } else if ok { info!("Sent MEMPOOL to {}", peer_addr); }
            
            if !ok {
                 log::error!("One or more critical initial requests failed for peer {}. Session will terminate after cleanup.", peer_addr);
            }
            initial_requests_successfully_sent = ok;

        } else {
            error!("Peer {} has no version info. Terminating session.", peer_addr);
        }
        
        if !initial_requests_successfully_sent {
            log::warn!("Initial P2P request sequence not successfully completed for peer {}. Terminating session.", peer_addr);
        } else {
            log::info!("Initial P2P request sequence completed for peer {}. Entering main message loop.", peer_addr);
            let mut sync_status_interval = time::interval(Duration::from_secs(SYNC_STATUS_INTERVAL_SECONDS));
            let mut last_block_count = 0u32;
            let mut last_pending_count = 0usize;
            
            loop {
                tokio::select! {
                    biased; 
                    read_result = read_network_message(&mut stream) => {
                        match read_result {
                            Ok((header, payload)) => {
                                let cmd_str = String::from_utf8_lossy(&header.command).trim_end_matches('\0').to_string();
                                debug!("Received message from {}: CMD=\"{}\", len={}", peer_addr, cmd_str, header.length);
    
                                if header.command == *CMD_HEADERS { // Handle HEADERS
                                    match HeadersMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(headers_msg) => {
                                            let num_headers_received = headers_msg.headers.len();
                                            info!("Received HEADERS message from {} with {} headers.", peer_addr, num_headers_received);
                                            let mut new_headers_in_current_msg = 0;
                                            if num_headers_received > 0 {
                                                for header_data in headers_msg.headers {
                                                    let current_header_hash = header_data.get_hash();
                                                    if let Some(_block_index_arc) = chain_state.add_block_index(header_data.clone()) {
                                                        if !session_requested_block_hashes.contains(&current_header_hash) && 
                                                           !pending_block_hashes_to_download.contains(&current_header_hash) &&
                                                           pending_block_hashes_to_download.len() < MAX_PENDING_BLOCKS {
                                                            pending_block_hashes_to_download.push(current_header_hash);
                                                            new_headers_in_current_msg += 1;
                                                            debug!("Added {} to pending download queue ({} total)", hex::encode(current_header_hash), pending_block_hashes_to_download.len());
                                                        }
                                                    }
                                                }
                                                info!("Processed {} headers from {}, {} were added to download queue.", num_headers_received, peer_addr, new_headers_in_current_msg);
                                            }
                                            if num_headers_received == MAX_HEADERS_PER_MSG && new_headers_in_current_msg > 0 {
                                                if send_getheaders_message(&mut stream, chain_state.get_block_locator(), [0u8; 32]).await.is_err() {
                                                    error!("Failed to send subsequent GETHEADERS to {}. Terminating session.", peer_addr); break;
                                                }
                                                debug!("Sent subsequent GETHEADERS to {}", peer_addr);
                                            } else {
                                                if !pending_block_hashes_to_download.is_empty() {
                                                    let mut batch_to_request_hashes = Vec::new();
                                                    let mut inventory_for_getdata = Vec::new();
                                                    while let Some(hash_to_download) = pending_block_hashes_to_download.pop() { 
                                                        if !session_requested_block_hashes.contains(&hash_to_download) {
                                                            batch_to_request_hashes.push(hash_to_download);
                                                            if batch_to_request_hashes.len() >= MAX_BLOCKS_TO_REQUEST_PER_GETDATA { break; }
                                                        }
                                                    }
                                                    if !batch_to_request_hashes.is_empty() {
                                                        for hash_val in &batch_to_request_hashes {
                                                            session_requested_block_hashes.insert(*hash_val);
                                                            inventory_for_getdata.push(InventoryVector { inv_type: InventoryType::Block, hash: *hash_val });
                                                        }
                                                        info!("Requesting {} blocks via GETDATA from {}", inventory_for_getdata.len(), peer_addr);
                                                        let getdata_msg = GetDataMessage { inventory: inventory_for_getdata };
                                                        let mut getdata_payload_bytes = Vec::new();
                                                        if getdata_msg.consensus_encode(&mut Cursor::new(&mut getdata_payload_bytes)).is_ok() {
                                                            let checksum = calculate_checksum(&getdata_payload_bytes);
                                                            let getdata_header = MessageHeader::new(*CMD_GETDATA, getdata_payload_bytes.len() as u32, checksum);
                                                            if send_message(&mut stream, getdata_header, &getdata_payload_bytes).await.is_err() {
                                                                error!("Failed to send GETDATA for blocks to {}. Terminating session.", peer_addr); break;
                                                            } else {
                                                                last_request_time = std::time::Instant::now();
                                                            }
                                                        } else { error!("Failed to encode GETDATA for blocks for {}: {}", peer_addr, "Encode error"); }
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => error!("Failed to decode HEADERS message from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_BLOCK {
                                    match BlockMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(block_msg) => {
                                            let block_hash = block_msg.header.get_hash();
                                            info!("🧱 Received BLOCK message from {}: hash={}", peer_addr, hex::encode(block_hash));
                                            session_requested_block_hashes.remove(&block_hash); // Allow re-request if needed later, or manage via global state
                                            let calculated_merkle_root = block_msg.calculate_merkle_root();
                                            if calculated_merkle_root == block_msg.header.merkle_root {
                                                let mut sig_check_passed = true;
                                                if block_msg.is_likely_proof_of_stake() {
                                                    match block_msg.verify_pos_signature() {
                                                        Ok(is_valid) => { if !is_valid { warn!("PoS Block {} signature check FAILED.", hex::encode(block_hash)); sig_check_passed = false; } }
                                                        Err(e) => { error!("Error during PoS signature verification for block {}: {}", hex::encode(block_hash), e); sig_check_passed = false; }
                                                    }
                                                }
                                                if sig_check_passed {
                                                    // Pass block to parallel sync manager for metrics tracking
                                                    if let Some(ref psm) = parallel_sync_manager {
                                                        psm.handle_block_received(block_msg.clone(), peer_addr).await;
                                                    }
                                                    
                                                    // Pass block to hybrid sync manager
                                                    if let Some(ref hsm) = hybrid_sync_manager {
                                                        hsm.handle_block_received(block_msg.clone(), peer_addr).await;
                                                    }
                                                    
                                                    // Save block to storage
                                                    match storage.save_block(&block_msg) {
                                                        Ok(_) => {
                                                            info!("✅ Successfully saved block {} to DB.", hex::encode(block_hash));
                                                            
                                                            // Add block header to chain state
                                                            if let Some(block_index) = chain_state.add_block_index(block_msg.header.clone()) {
                                                                info!("🔗 Added block {} to chain state at height {} (tip updated)", 
                                                                      hex::encode(block_hash), block_index.height);
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!("Failed to save block {} to DB: {}", hex::encode(block_hash), e);
                                                        }
                                                    }
                                                } else { warn!("Block {} failed basic validation. Discarding.", hex::encode(block_hash)); }
                                            } else { warn!("Merkle root for block {} MISMATCH! Invalid block.", hex::encode(block_hash)); }
                                        }
                                        Err(e) => error!("Failed to decode BLOCK message from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_INV {
                                    match crate::p2p::messages::read_var_int(&mut Cursor::new(&payload)) { 
                                        Ok(count) => {
                                            let mut cursor_inv = Cursor::new(payload);
                                            let _ = crate::p2p::messages::read_var_int(&mut cursor_inv); // consume count again
                                            info!("📦 INV message from {} contains {} items.", peer_addr, count);
                                            if count > 50_000 { error!("INV from {} too many items.", peer_addr); break; }
                                            let mut vec_to_fetch: Vec<InventoryVector> = Vec::new();
                                            let mut inv_types_count: std::collections::HashMap<InventoryType, usize> = std::collections::HashMap::new();
                                            for _i in 0..count {
                                                match InventoryVector::consensus_decode(&mut cursor_inv) {
                                                    Ok(inv_item) => { 
                                                        *inv_types_count.entry(inv_item.inv_type).or_insert(0) += 1;
                                                        debug!("INV item from {}: type={:?}, hash={}", peer_addr, inv_item.inv_type, hex::encode(inv_item.hash));
                                                        if inv_item.inv_type != InventoryType::Error { 
                                                            vec_to_fetch.push(inv_item); 
                                                        }
                                                    }
                                                    Err(e) => { error!("Failed to decode inv item from {}: {}.", peer_addr, e); break; }
                                                }
                                            }
                                            info!("📊 INV types from {}: {:?}", peer_addr, inv_types_count);
                                            
                                            // Pass INV to hybrid sync manager
                                            if let Some(ref hsm) = hybrid_sync_manager {
                                                hsm.handle_inv_received(vec_to_fetch.clone(), peer_addr).await;
                                            }
                                            
                                            if !vec_to_fetch.is_empty() {
                                                info!("🔄 Requesting {} items via GETDATA from {} (blocks: {})", 
                                                      vec_to_fetch.len(), peer_addr, 
                                                      vec_to_fetch.iter().filter(|iv| iv.inv_type == InventoryType::Block).count());
                                                
                                                // Track requested blocks to prevent duplicates
                                                for inv_item in &vec_to_fetch {
                                                    if inv_item.inv_type == InventoryType::Block {
                                                        session_requested_block_hashes.insert(inv_item.hash);
                                                    }
                                                }
                                                
                                                let getdata_msg = GetDataMessage { inventory: vec_to_fetch };
                                                let mut getdata_payload_bytes = Vec::new();
                                                if getdata_msg.consensus_encode(&mut Cursor::new(&mut getdata_payload_bytes)).is_ok() {
                                                    let checksum = calculate_checksum(&getdata_payload_bytes);
                                                    let getdata_header = MessageHeader::new(*CMD_GETDATA, getdata_payload_bytes.len() as u32, checksum);
                                                    if send_message(&mut stream, getdata_header, &getdata_payload_bytes).await.is_err() {
                                                        error!("Failed to send GETDATA to {}: {}. Terminating.", peer_addr, "Send error"); break;
                                                    } else {
                                                        debug!("Successfully sent GETDATA to {}", peer_addr);
                                                        last_request_time = std::time::Instant::now();
                                                    }
                                                } else { error!("Failed to encode GETDATA for {}: {}", peer_addr, "Encode error"); }
                                            } else {
                                                debug!("No items to fetch from INV message from {}", peer_addr);
                                            }
                                        }
                                        Err(e) => { error!("Failed to read inv count from {}: {}.", peer_addr, e); break; }
                                    }
                                } else if header.command == *CMD_MNB { 
                                    match MasternodeBroadcast::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(mnb_msg) => masternode_manager.process_mnb_message(mnb_msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode MNB from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_SPORK { 
                                    match SporkMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(spork_msg) => spork_manager.process_spork_message(spork_msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode SPORK from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_MPROP {
                                    match BudgetProposalBroadcast::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => governance_manager.process_mprop_message(msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode MPROP from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_MVOTE {
                                    match BudgetVote::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => governance_manager.process_mvote_message(msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode MVOTE from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_FBS {
                                    match FinalizedBudgetBroadcast::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => governance_manager.process_fbs_message(msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode FBS from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_FBVOTE {
                                    match FinalizedBudgetVote::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => governance_manager.process_fbvote_message(msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode FBVOTE from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_IX {
                                    match TxLockRequest::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => instant_send_manager.process_ix_message(msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode IX from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_TXLVOTE {
                                    match ConsensusVote::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => instant_send_manager.process_txlvote_message(msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode TXLVOTE from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_MNP { 
                                    match MasternodePing::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(mnp_msg) => masternode_manager.process_mnp_message(mnp_msg, Some(peer_addr)),
                                        Err(e) => error!("Failed to decode MNP from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_DSTX { 
                                    match DstxMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => info!("DSTX from {}: txid={}", peer_addr, hex::encode(msg.tx.get_txid())),
                                        Err(e) => error!("Failed to decode DSTX from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_MNW {
                                    match MasternodeWinner::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => info!("MNW from {}: BlockHeight={}", peer_addr, msg.n_block_height),
                                        Err(e) => error!("Failed to decode MNW from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_SSC {
                                    match SscMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(msg) => info!("SSC from {}: ItemID={}, Count={}", peer_addr, msg.item_id, msg.count),
                                        Err(e) => error!("Failed to decode SSC from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_ADDR {
                                    match AddrMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(addr_msg) => {
                                            info!("ADDR from {}: Got {} addresses.", peer_addr, addr_msg.addresses.len());
                                            // TODO: Pass to AddrManager
                                        }
                                        Err(e) => error!("Failed to decode ADDR from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_REJECT {
                                    match RejectMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(reject_msg) => error!("Peer {} rejected: cmd='{}', code={}, reason='{}', hash={}", peer_addr, reject_msg.message_cmd, reject_msg.code, reject_msg.reason, hex::encode(reject_msg.data_hash)),
                                        Err(e) => error!("Failed to decode REJECT from {}: {}", peer_addr, e),
                                    }
                                } else if header.command == *CMD_NOTFOUND {
                                    match crate::p2p::messages::read_var_int(&mut Cursor::new(&payload)) { 
                                        Ok(count) => {
                                            let mut cursor_nf = Cursor::new(payload);
                                            let _ = crate::p2p::messages::read_var_int(&mut cursor_nf); // consume count
                                            warn!("NOTFOUND from {} for {} items - peers don't have requested data!", peer_addr, count);
                                            for _i in 0..count {
                                                match InventoryVector::consensus_decode(&mut cursor_nf) {
                                                    Ok(inv_item) => error!("Item NOT FOUND by {}: type={:?}, hash={}", peer_addr, inv_item.inv_type, hex::encode(inv_item.hash)),
                                                    Err(e) => { error!("Failed to decode notfound item from {}: {}.", peer_addr, e); break; }
                                                }
                                            }
                                        }
                                        Err(e) => { error!("Failed to read notfound count from {}: {}.", peer_addr, e); break; }
                                    }
                                } else if header.command == *CMD_PING {
                                    match PingMessage::consensus_decode(&mut Cursor::new(&payload)) {
                                        Ok(ping_msg) => {
                                            let pong_payload = PongMessage::new(ping_msg.nonce);
                                            let mut pong_payload_bytes = Vec::new();
                                            if pong_payload.consensus_encode(&mut Cursor::new(&mut pong_payload_bytes)).is_ok() {
                                                let pong_checksum = calculate_checksum(&pong_payload_bytes);
                                                let pong_header = MessageHeader::new(*CMD_PONG, pong_payload_bytes.len() as u32, pong_checksum);
                                                if send_message(&mut stream, pong_header, &pong_payload_bytes).await.is_err() {
                                                    error!("Failed to send PONG to {}: {}. Terminating.", peer_addr, "Send error"); break;
                                                }
                                            }
                                        }
                                        Err(e) => error!("Failed to decode PING from {}: {}", peer_addr, e),
                                    }
                                } else {
                                    debug!("Unhandled message CMD: \"{}\" from {}", cmd_str, peer_addr);
                                }
                            }
                            Err(e) => {
                                if e.kind() == IoErrorKind::UnexpectedEof || e.kind() == IoErrorKind::ConnectionReset {
                                    info!("Connection with {} closed by peer or EOF.", peer_addr);
                                } else {
                                    error!("Error reading message from {}: {}", peer_addr, e);
                                }
                                break; 
                            }
                        }
                    }
                    _ = ping_interval.tick() => {
                        let ping_nonce: u64 = rand::random();
                        let ping_payload = PingMessage::new(ping_nonce);
                        let mut ping_payload_bytes = Vec::new();
                        if ping_payload.consensus_encode(&mut std::io::Cursor::new(&mut ping_payload_bytes)).is_ok() {
                            let ping_checksum = calculate_checksum(&ping_payload_bytes);
                            let ping_header = MessageHeader::new(*CMD_PING, ping_payload_bytes.len() as u32, ping_checksum);
                            if let Err(e) = send_message(&mut stream, ping_header, &ping_payload_bytes).await {
                                error!("Failed to send periodic PING to {}: {}. Terminating session.", peer_addr, e);
                                break;
                            }
                            debug!("Sent periodic PING (nonce: {}) to {}", ping_nonce, peer_addr);
                        } else {
                            error!("Failed to encode periodic PING for {}. Terminating session.", peer_addr);
                            break;
                        }
                    }
                    _ = sync_status_interval.tick() => {
                        // Periodic sync status reporting
                        let current_tip = chain_state.get_tip();
                        let current_block_count = current_tip.as_ref().map_or(0, |tip| tip.height);
                        let current_pending_count = pending_block_hashes_to_download.len();
                        let current_requested_count = session_requested_block_hashes.len();
                        let peer_count = peer_manager_arc.get_peer_count();
                        
                        let blocks_synced_since_last = current_block_count - last_block_count;
                        let pending_change = current_pending_count as i32 - last_pending_count as i32;
                        
                        info!("SYNC STATUS [{}]: Height={} (+{}), Pending={} ({:+}), Requested={}, Peers={}", 
                              peer_addr, current_block_count, blocks_synced_since_last, 
                              current_pending_count, pending_change, current_requested_count, peer_count);
                        
                        // Enhanced stall detection - more aggressive recovery
                        let request_timeout = last_request_time.elapsed().as_secs() > REQUEST_TIMEOUT_SECONDS;
                        let is_stalled = blocks_synced_since_last == 0 && 
                                       (current_pending_count == 0 || current_requested_count == 0 || request_timeout);
                        
                        if is_stalled {
                            warn!("SYNC STALLED [{}]: No progress in {}s (Height={}, Pending={}, Requested={}) - recovering", 
                                  peer_addr, SYNC_STATUS_INTERVAL_SECONDS, current_block_count, current_pending_count, current_requested_count);
                            
                            // Clear stale requests that might be blocking progress
                            if current_requested_count > 0 || request_timeout {
                                info!("Clearing {} stale block requests from {} (timeout: {})", 
                                      current_requested_count, peer_addr, request_timeout);
                                session_requested_block_hashes.clear();
                                last_request_time = std::time::Instant::now();
                            }
                            
                            // Try to restart sync by sending another GETBLOCKS
                            if send_getblocks_message(&mut stream, chain_state.get_block_locator(), [0u8; 32]).await.is_err() {
                                error!("Failed to send recovery GETBLOCKS to {}. Terminating session.", peer_addr);
                                break;
                            }
                            info!("Sent recovery GETBLOCKS to {} to restart sync", peer_addr);
                        }
                        
                        // Proactive block requesting if we have pending blocks but no active requests
                        else if current_pending_count > 0 && current_requested_count == 0 {
                            info!("Proactively requesting blocks from {} (Pending={}, Requested={})", 
                                  peer_addr, current_pending_count, current_requested_count);
                            
                            // Request a batch of blocks immediately
                            let mut batch_to_request_hashes = Vec::new();
                            let mut inventory_for_getdata = Vec::new();
                            let request_count = std::cmp::min(pending_block_hashes_to_download.len(), MAX_BLOCKS_TO_REQUEST_PER_GETDATA);
                            
                            for _ in 0..request_count {
                                if let Some(hash_to_download) = pending_block_hashes_to_download.pop() {
                                    if !session_requested_block_hashes.contains(&hash_to_download) {
                                        batch_to_request_hashes.push(hash_to_download);
                                        session_requested_block_hashes.insert(hash_to_download);
                                        inventory_for_getdata.push(InventoryVector { inv_type: InventoryType::Block, hash: hash_to_download });
                                    }
                                }
                            }
                            
                            if !inventory_for_getdata.is_empty() {
                                info!("Proactively requesting {} blocks via GETDATA from {}", inventory_for_getdata.len(), peer_addr);
                                let getdata_msg = GetDataMessage { inventory: inventory_for_getdata };
                                let mut getdata_payload_bytes = Vec::new();
                                if getdata_msg.consensus_encode(&mut Cursor::new(&mut getdata_payload_bytes)).is_ok() {
                                    let checksum = calculate_checksum(&getdata_payload_bytes);
                                    let getdata_header = MessageHeader::new(*CMD_GETDATA, getdata_payload_bytes.len() as u32, checksum);
                                                                     if send_message(&mut stream, getdata_header, &getdata_payload_bytes).await.is_err() {
                                     error!("Failed to send proactive GETDATA to {}. Terminating session.", peer_addr);
                                     break;
                                 } else {
                                     last_request_time = std::time::Instant::now();
                                 }
                                }
                            }
                        }
                        
                        last_block_count = current_block_count;
                        last_pending_count = current_pending_count;
                    }
                }
            }
        }
    } else {
        warn!("Stream already taken for peer {}, cannot start session handling.", peer_addr);
    }

    info!("Peer session ended for {}. Removing from manager.", peer_addr);
    if let Ok(mut peer_guard) = peer_arc.try_lock() {
        if let Some(mut s) = peer_guard.stream.take() { 
            let _ = s.shutdown().await;
        }
        peer_guard.connected = false;
        peer_guard.handshake_completed = false; 
    } else {
        warn!("Could not acquire lock to fully clean up peer {} state during removal.", peer_addr);
    }
    
    // Remove peer from parallel sync manager
    if let Some(ref psm) = parallel_sync_manager {
        psm.remove_peer(peer_addr).await;
    }
    
    // Remove peer from hybrid sync manager
    if let Some(ref hsm) = hybrid_sync_manager {
        hsm.unregister_peer(&peer_addr).await;
    }
    
    peer_manager_arc.remove_peer(&peer_addr);
} 