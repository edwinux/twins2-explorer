use std::sync::Arc;
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::sync::{Mutex as TokioMutex, RwLock as TokioRwLock};
use crate::p2p::messages::{BlockMessage, GetDataMessage, InventoryVector, InventoryType};
use crate::blockchain::chain_state::ChainState;
use crate::blockchain::orphan_manager::OrphanManager;
use crate::storage::BlockStorage;
use log::{info, debug, warn, error};
use std::time::{Duration, Instant};

// Protocol extension for Rust nodes
const RUST_NODE_IDENTIFIER: &str = "TWINS-Rust";
const FAST_SYNC_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum PeerType {
    StandardCpp,    // Original C++ TWINS nodes
    RustFastSync,   // New Rust nodes with fast sync support
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_type: PeerType,
    pub user_agent: String,
    pub fast_sync_version: Option<u32>,
    pub last_seen: Instant,
    pub blocks_received: u64,
    pub average_response_time: Duration,
}

pub struct HybridSyncManager {
    chain_state: Arc<ChainState>,
    storage: Arc<dyn BlockStorage>,
    orphan_manager: Arc<OrphanManager>,
    
    // Peer tracking
    peer_info: Arc<TokioRwLock<HashMap<std::net::SocketAddr, PeerInfo>>>,
    rust_peers: Arc<TokioRwLock<HashSet<std::net::SocketAddr>>>,
    
    // Sync mode
    sync_mode: Arc<TokioRwLock<SyncMode>>,
    
    // Standard sync state (for C++ nodes)
    pending_blocks: Arc<TokioMutex<HashMap<[u8; 32], Instant>>>,
    block_queue: Arc<TokioMutex<VecDeque<BlockMessage>>>,
    
    // Fast sync state (for Rust nodes)
    fast_sync_active: Arc<TokioMutex<bool>>,
    segment_size: usize,
    
    // Statistics
    total_blocks_received: Arc<TokioMutex<u64>>,
    total_blocks_processed: Arc<TokioMutex<u64>>,
    sync_start_time: Arc<TokioMutex<Option<Instant>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyncMode {
    Standard,       // Traditional sync from C++ nodes
    Fast,          // Fast parallel sync from Rust nodes
    Hybrid,        // Mix of both based on peer availability
}

impl HybridSyncManager {
    pub fn new(
        chain_state: Arc<ChainState>,
        storage: Arc<dyn BlockStorage>,
        orphan_manager: Arc<OrphanManager>,
    ) -> Self {
        Self {
            chain_state,
            storage,
            orphan_manager,
            peer_info: Arc::new(TokioRwLock::new(HashMap::new())),
            rust_peers: Arc::new(TokioRwLock::new(HashSet::new())),
            sync_mode: Arc::new(TokioRwLock::new(SyncMode::Standard)),
            pending_blocks: Arc::new(TokioMutex::new(HashMap::new())),
            block_queue: Arc::new(TokioMutex::new(VecDeque::new())),
            fast_sync_active: Arc::new(TokioMutex::new(false)),
            segment_size: 1000,
            total_blocks_received: Arc::new(TokioMutex::new(0)),
            total_blocks_processed: Arc::new(TokioMutex::new(0)),
            sync_start_time: Arc::new(TokioMutex::new(None)),
        }
    }
    
    pub async fn start(&self) {
        info!("🚀 Starting Hybrid Sync Manager");
        
        // Set start time
        {
            let mut start_time = self.sync_start_time.lock().await;
            *start_time = Some(Instant::now());
        }
        
        // Start the block processor
        self.start_block_processor().await;
        
        // Start the sync mode monitor
        self.start_sync_mode_monitor().await;
        
        // Start the statistics reporter
        self.start_stats_reporter().await;
    }
    
    pub async fn register_peer(&self, addr: std::net::SocketAddr, user_agent: &str, _version: i32) {
        let peer_type = if user_agent.contains(RUST_NODE_IDENTIFIER) {
            // This is a Rust node - check if it supports fast sync
            let fast_sync_version = Self::extract_fast_sync_version(user_agent);
            if fast_sync_version.is_some() {
                PeerType::RustFastSync
            } else {
                PeerType::StandardCpp // Rust node without fast sync
            }
        } else {
            PeerType::StandardCpp
        };
        
        let peer_info = PeerInfo {
            peer_type: peer_type.clone(),
            user_agent: user_agent.to_string(),
            fast_sync_version: if peer_type == PeerType::RustFastSync {
                Self::extract_fast_sync_version(user_agent)
            } else {
                None
            },
            last_seen: Instant::now(),
            blocks_received: 0,
            average_response_time: Duration::from_millis(100),
        };
        
        {
            let mut peers = self.peer_info.write().await;
            peers.insert(addr, peer_info);
        }
        
        if peer_type == PeerType::RustFastSync {
            let mut rust_peers = self.rust_peers.write().await;
            rust_peers.insert(addr);
            info!("🦀 Discovered Rust peer with fast sync support: {} (v{})", 
                  addr, Self::extract_fast_sync_version(user_agent).unwrap_or(0));
        }
        
        // Update sync mode based on available peers
        self.update_sync_mode().await;
    }
    
    pub async fn unregister_peer(&self, addr: &std::net::SocketAddr) {
        {
            let mut peers = self.peer_info.write().await;
            peers.remove(addr);
        }
        
        {
            let mut rust_peers = self.rust_peers.write().await;
            rust_peers.remove(addr);
        }
        
        // Update sync mode when peers disconnect
        self.update_sync_mode().await;
    }
    
    pub async fn handle_block_received(&self, block: BlockMessage, from_peer: std::net::SocketAddr) {
        let block_hash = block.header.get_hash();
        
        // Update peer statistics
        {
            let mut peers = self.peer_info.write().await;
            if let Some(peer_info) = peers.get_mut(&from_peer) {
                peer_info.blocks_received += 1;
                peer_info.last_seen = Instant::now();
            }
        }
        
        // Update total received
        {
            let mut total = self.total_blocks_received.lock().await;
            *total += 1;
        }
        
        // Remove from pending if it was requested
        {
            let mut pending = self.pending_blocks.lock().await;
            pending.remove(&block_hash);
        }
        
        // Queue for processing
        {
            let mut queue = self.block_queue.lock().await;
            queue.push_back(block);
        }
        
        debug!("📦 Received block {} from {} (queue size: {})", 
               hex::encode(&block_hash[..8]), from_peer, self.block_queue.lock().await.len());
    }
    
    pub async fn handle_inv_received(&self, inv_items: Vec<InventoryVector>, from_peer: std::net::SocketAddr) {
        let block_invs: Vec<[u8; 32]> = inv_items.into_iter()
            .filter(|inv| inv.inv_type == InventoryType::Block)
            .map(|inv| inv.hash)
            .collect();
        
        if block_invs.is_empty() {
            return;
        }
        
        info!("📋 Received INV with {} blocks from {}", block_invs.len(), from_peer);
        
        // In standard mode, just track what we need
        let mode = self.sync_mode.read().await.clone();
        match mode {
            SyncMode::Standard => {
                // For C++ nodes, we'll request blocks normally through peer_manager
                // Just log for now
                debug!("Standard sync mode - INV will be handled by peer_manager");
            }
            SyncMode::Fast | SyncMode::Hybrid => {
                // For fast sync, we could batch these requests
                // But for now, let standard sync handle it
                debug!("Fast/Hybrid sync mode - INV processing not yet implemented");
            }
        }
    }
    
    async fn update_sync_mode(&self) {
        let rust_peer_count = self.rust_peers.read().await.len();
        let total_peer_count = self.peer_info.read().await.len();
        
        let new_mode = if rust_peer_count >= 3 {
            // If we have at least 3 Rust peers, use fast sync
            SyncMode::Fast
        } else if rust_peer_count > 0 && total_peer_count > 3 {
            // If we have some Rust peers and enough total peers, use hybrid
            SyncMode::Hybrid
        } else {
            // Otherwise, use standard sync
            SyncMode::Standard
        };
        
        let mut current_mode = self.sync_mode.write().await;
        if *current_mode != new_mode {
            info!("🔄 Switching sync mode from {:?} to {:?} (Rust peers: {}, Total: {})",
                  *current_mode, new_mode, rust_peer_count, total_peer_count);
            *current_mode = new_mode;
        }
    }
    
    async fn start_block_processor(&self) {
        let block_queue = Arc::clone(&self.block_queue);
        let orphan_manager = Arc::clone(&self.orphan_manager);
        let total_processed = Arc::clone(&self.total_blocks_processed);
        
        // Start multiple processing threads
        for thread_id in 0..4 {
            let block_queue = Arc::clone(&block_queue);
            let orphan_manager = Arc::clone(&orphan_manager);
            let total_processed = Arc::clone(&total_processed);
            
            tokio::spawn(async move {
                loop {
                    // Get next block to process
                    let block_opt = {
                        let mut queue = block_queue.lock().await;
                        queue.pop_front()
                    };
                    
                    if let Some(block) = block_opt {
                        let block_hash = block.header.get_hash();
                        let start = Instant::now();
                        
                        // Process through orphan manager
                        match orphan_manager.add_block(block).await {
                            Ok(_) => {
                                let mut processed = total_processed.lock().await;
                                *processed += 1;
                                
                                debug!("Thread {} processed block {} in {:?}", 
                                       thread_id, hex::encode(&block_hash[..8]), start.elapsed());
                            }
                            Err(e) => {
                                error!("Thread {} failed to process block {}: {}", 
                                       thread_id, hex::encode(&block_hash[..8]), e);
                            }
                        }
                    } else {
                        // No blocks to process, sleep briefly
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            });
        }
    }
    
    async fn start_sync_mode_monitor(&self) {
        let sync_mode = Arc::clone(&self.sync_mode);
        let rust_peers = Arc::clone(&self.rust_peers);
        let fast_sync_active = Arc::clone(&self.fast_sync_active);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                let mode = sync_mode.read().await.clone();
                let rust_peer_count = rust_peers.read().await.len();
                
                match mode {
                    SyncMode::Fast => {
                        let mut active = fast_sync_active.lock().await;
                        if !*active && rust_peer_count >= 3 {
                            info!("🚀 Activating FAST SYNC with {} Rust peers!", rust_peer_count);
                            *active = true;
                            // TODO: Implement fast sync protocol negotiation
                        }
                    }
                    SyncMode::Hybrid => {
                        info!("🔀 Running in HYBRID mode with {} Rust peers", rust_peer_count);
                        // TODO: Implement hybrid sync logic
                    }
                    SyncMode::Standard => {
                        let mut active = fast_sync_active.lock().await;
                        if *active {
                            info!("📉 Deactivating fast sync - not enough Rust peers");
                            *active = false;
                        }
                    }
                }
            }
        });
    }
    
    async fn start_stats_reporter(&self) {
        let total_received = Arc::clone(&self.total_blocks_received);
        let total_processed = Arc::clone(&self.total_blocks_processed);
        let sync_start_time = Arc::clone(&self.sync_start_time);
        let sync_mode = Arc::clone(&self.sync_mode);
        let peer_info = Arc::clone(&self.peer_info);
        let block_queue = Arc::clone(&self.block_queue);
        let orphan_manager = Arc::clone(&self.orphan_manager);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let received = *total_received.lock().await;
                let processed = *total_processed.lock().await;
                let queue_size = block_queue.lock().await.len();
                let orphan_count = orphan_manager.get_orphan_count().await;
                
                let elapsed = {
                    let start = sync_start_time.lock().await;
                    start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0)
                };
                
                let blocks_per_sec = if elapsed > 0.0 {
                    processed as f64 / elapsed
                } else {
                    0.0
                };
                
                let mode = sync_mode.read().await.clone();
                let peers = peer_info.read().await;
                let rust_count = peers.values().filter(|p| p.peer_type == PeerType::RustFastSync).count();
                let cpp_count = peers.values().filter(|p| p.peer_type == PeerType::StandardCpp).count();
                
                info!("🔄 HYBRID SYNC [{:?}]: {:.0} blk/s | Processed: {} | Queue: {} | Orphans: {} | Peers: {}R+{}C",
                      mode, blocks_per_sec, processed, queue_size, orphan_count, rust_count, cpp_count);
            }
        });
    }
    
    fn extract_fast_sync_version(user_agent: &str) -> Option<u32> {
        // Extract fast sync version from user agent string
        // Format: "TWINS-Rust/0.1.0/FastSync:1"
        if let Some(pos) = user_agent.find("FastSync:") {
            let version_str = &user_agent[pos + 9..];
            if let Some(end) = version_str.find(|c: char| !c.is_numeric()) {
                version_str[..end].parse().ok()
            } else {
                version_str.parse().ok()
            }
        } else {
            None
        }
    }
    
    pub async fn get_sync_status(&self) -> HybridSyncStatus {
        let mode = self.sync_mode.read().await.clone();
        let received = *self.total_blocks_received.lock().await;
        let processed = *self.total_blocks_processed.lock().await;
        let queue_size = self.block_queue.lock().await.len();
        let orphan_count = self.orphan_manager.get_orphan_count().await;
        
        let elapsed = {
            let start = self.sync_start_time.lock().await;
            start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0)
        };
        
        let blocks_per_sec = if elapsed > 0.0 {
            processed as f64 / elapsed
        } else {
            0.0
        };
        
        let peers = self.peer_info.read().await;
        let rust_peer_count = peers.values().filter(|p| p.peer_type == PeerType::RustFastSync).count();
        let cpp_peer_count = peers.values().filter(|p| p.peer_type == PeerType::StandardCpp).count();
        let fast_sync_active = *self.fast_sync_active.lock().await;
        
        HybridSyncStatus {
            sync_mode: mode,
            total_blocks_received: received,
            total_blocks_processed: processed,
            blocks_in_queue: queue_size as u64,
            orphan_blocks: orphan_count as u64,
            blocks_per_second: blocks_per_sec,
            elapsed_seconds: elapsed,
            rust_peer_count: rust_peer_count as u32,
            cpp_peer_count: cpp_peer_count as u32,
            fast_sync_active,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HybridSyncStatus {
    pub sync_mode: SyncMode,
    pub total_blocks_received: u64,
    pub total_blocks_processed: u64,
    pub blocks_in_queue: u64,
    pub orphan_blocks: u64,
    pub blocks_per_second: f64,
    pub elapsed_seconds: f64,
    pub rust_peer_count: u32,
    pub cpp_peer_count: u32,
    pub fast_sync_active: bool,
}
