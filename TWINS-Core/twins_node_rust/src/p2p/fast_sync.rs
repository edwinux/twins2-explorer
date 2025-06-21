use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::mpsc;
use crate::p2p::messages::{BlockMessage, GetDataMessage, InventoryVector, InventoryType};
use crate::blockchain::chain_state::ChainState;
use crate::blockchain::orphan_manager::OrphanManager;
use crate::storage::BlockStorage;
use log::{info, debug, warn, error};
use std::time::{Duration, Instant};

const MAX_CONCURRENT_REQUESTS: usize = 50;
const BLOCKS_PER_REQUEST: usize = 500;
const REQUEST_TIMEOUT_SECS: u64 = 30;
const MAX_RETRIES: u32 = 3;

#[derive(Clone)]
pub struct BlockRequest {
    pub peer_addr: std::net::SocketAddr,
    pub block_hashes: Vec<[u8; 32]>,
    pub requested_at: Instant,
    pub retry_count: u32,
}

pub struct FastSync {
    chain_state: Arc<ChainState>,
    storage: Arc<dyn BlockStorage>,
    orphan_manager: Arc<OrphanManager>,
    
    // Pending block requests
    pending_requests: Arc<TokioMutex<HashMap<[u8; 32], BlockRequest>>>,
    
    // Blocks we need to download
    blocks_to_download: Arc<TokioMutex<VecDeque<[u8; 32]>>>,
    
    // Downloaded blocks waiting to be processed
    downloaded_blocks: Arc<TokioMutex<VecDeque<BlockMessage>>>,
    
    // Channel to send GETDATA requests to peers
    request_sender: mpsc::Sender<(std::net::SocketAddr, GetDataMessage)>,
    request_receiver: Arc<TokioMutex<mpsc::Receiver<(std::net::SocketAddr, GetDataMessage)>>>,
    
    // Statistics
    total_requested: Arc<TokioMutex<u64>>,
    total_received: Arc<TokioMutex<u64>>,
    total_processed: Arc<TokioMutex<u64>>,
    start_time: Arc<TokioMutex<Option<Instant>>>,
}

impl FastSync {
    pub fn new(
        chain_state: Arc<ChainState>,
        storage: Arc<dyn BlockStorage>,
        orphan_manager: Arc<OrphanManager>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(100);
        
        Self {
            chain_state,
            storage,
            orphan_manager,
            pending_requests: Arc::new(TokioMutex::new(HashMap::new())),
            blocks_to_download: Arc::new(TokioMutex::new(VecDeque::new())),
            downloaded_blocks: Arc::new(TokioMutex::new(VecDeque::new())),
            request_sender: tx,
            request_receiver: Arc::new(TokioMutex::new(rx)),
            total_requested: Arc::new(TokioMutex::new(0)),
            total_received: Arc::new(TokioMutex::new(0)),
            total_processed: Arc::new(TokioMutex::new(0)),
            start_time: Arc::new(TokioMutex::new(None)),
        }
    }
    
    pub async fn start(&self, target_height: u32) {
        info!("🚀 Starting FastSync to height {}", target_height);
        
        // Set start time
        {
            let mut start_time = self.start_time.lock().await;
            *start_time = Some(Instant::now());
        }
        
        // Start the request dispatcher
        self.start_request_dispatcher().await;
        
        // Start the block processor
        self.start_block_processor().await;
        
        // Start the statistics reporter
        self.start_stats_reporter().await;
        
        // Start the timeout checker
        self.start_timeout_checker().await;
    }
    
    pub async fn add_blocks_to_download(&self, block_hashes: Vec<[u8; 32]>) {
        let mut queue = self.blocks_to_download.lock().await;
        let pending = self.pending_requests.lock().await;
        
        for hash in block_hashes {
            // Skip if already pending or downloaded
            if !pending.contains_key(&hash) && !self.storage.has_block(&hash).unwrap_or(false) {
                queue.push_back(hash);
            }
        }
        
        debug!("Added {} blocks to download queue", queue.len());
    }
    
    pub async fn handle_block_received(&self, block: BlockMessage, from_peer: std::net::SocketAddr) {
        let block_hash = block.header.get_hash();
        
        // Update statistics
        {
            let mut received = self.total_received.lock().await;
            *received += 1;
        }
        
        // Remove from pending requests
        {
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&block_hash);
        }
        
        // Add to downloaded blocks queue
        {
            let mut downloaded = self.downloaded_blocks.lock().await;
            downloaded.push_back(block);
        }
        
        debug!("📦 Received block {} from {}", hex::encode(&block_hash[..8]), from_peer);
    }
    
    pub fn get_request_receiver(&self) -> Arc<TokioMutex<mpsc::Receiver<(std::net::SocketAddr, GetDataMessage)>>> {
        Arc::clone(&self.request_receiver)
    }
    
    async fn start_request_dispatcher(&self) {
        let blocks_to_download = Arc::clone(&self.blocks_to_download);
        let pending_requests = Arc::clone(&self.pending_requests);
        let request_sender = self.request_sender.clone();
        let total_requested = Arc::clone(&self.total_requested);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            let mut peer_index = 0;
            
            // Simulated peer list - in real implementation, get from PeerManager
            let peers = vec![
                "65.20.69.84:37817".parse().unwrap(),
                "158.247.239.189:37817".parse().unwrap(),
                "134.209.227.149:37817".parse().unwrap(),
                "159.65.84.118:37817".parse().unwrap(),
                "104.238.185.162:37817".parse().unwrap(),
                "208.85.21.121:37817".parse().unwrap(),
                "167.99.223.138:37817".parse().unwrap(),
                "68.183.161.44:37817".parse().unwrap(),
                "45.77.118.66:37817".parse().unwrap(),
                "142.93.145.197:37817".parse().unwrap(),
                "216.128.182.65:37817".parse().unwrap(),
                "67.219.107.45:37817".parse().unwrap(),
                "45.32.36.145:37817".parse().unwrap(),
            ];
            
            loop {
                interval.tick().await;
                
                let pending_count = pending_requests.lock().await.len();
                if pending_count >= MAX_CONCURRENT_REQUESTS {
                    continue;
                }
                
                // Create batch of blocks to request
                let mut batch = Vec::new();
                {
                    let mut queue = blocks_to_download.lock().await;
                    while batch.len() < BLOCKS_PER_REQUEST && !queue.is_empty() {
                        if let Some(hash) = queue.pop_front() {
                            batch.push(hash);
                        }
                    }
                }
                
                if batch.is_empty() {
                    continue;
                }
                
                // Select peer round-robin
                let peer_addr = peers[peer_index % peers.len()];
                peer_index += 1;
                
                // Create GETDATA message
                let inventory: Vec<InventoryVector> = batch.iter()
                    .map(|hash| InventoryVector {
                        inv_type: InventoryType::Block,
                        hash: *hash,
                    })
                    .collect();
                
                let getdata_msg = GetDataMessage { inventory };
                
                // Record pending requests
                {
                    let mut pending = pending_requests.lock().await;
                    let request = BlockRequest {
                        peer_addr,
                        block_hashes: batch.clone(),
                        requested_at: Instant::now(),
                        retry_count: 0,
                    };
                    
                    for hash in &batch {
                        pending.insert(*hash, request.clone());
                    }
                    
                    let mut requested = total_requested.lock().await;
                    *requested += batch.len() as u64;
                }
                
                // Send request
                if let Err(e) = request_sender.send((peer_addr, getdata_msg)).await {
                    error!("Failed to send GETDATA request: {}", e);
                } else {
                    debug!("📤 Requested {} blocks from {}", batch.len(), peer_addr);
                }
            }
        });
    }
    
    async fn start_block_processor(&self) {
        let downloaded_blocks = Arc::clone(&self.downloaded_blocks);
        let orphan_manager = Arc::clone(&self.orphan_manager);
        let total_processed = Arc::clone(&self.total_processed);
        
        // Start multiple processing threads
        for thread_id in 0..8 {
            let downloaded_blocks = Arc::clone(&downloaded_blocks);
            let orphan_manager = Arc::clone(&orphan_manager);
            let total_processed = Arc::clone(&total_processed);
            
            tokio::spawn(async move {
                loop {
                    // Get next block to process
                    let block_opt = {
                        let mut queue = downloaded_blocks.lock().await;
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
    
    async fn start_stats_reporter(&self) {
        let total_requested = Arc::clone(&self.total_requested);
        let total_received = Arc::clone(&self.total_received);
        let total_processed = Arc::clone(&self.total_processed);
        let start_time = Arc::clone(&self.start_time);
        let pending_requests = Arc::clone(&self.pending_requests);
        let downloaded_blocks = Arc::clone(&self.downloaded_blocks);
        let orphan_manager = Arc::clone(&self.orphan_manager);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let requested = *total_requested.lock().await;
                let received = *total_received.lock().await;
                let processed = *total_processed.lock().await;
                let pending = pending_requests.lock().await.len();
                let queued = downloaded_blocks.lock().await.len();
                let orphans = orphan_manager.get_orphan_count().await;
                
                let elapsed = {
                    let start = start_time.lock().await;
                    start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0)
                };
                
                let blocks_per_sec = if elapsed > 0.0 {
                    processed as f64 / elapsed
                } else {
                    0.0
                };
                
                info!("⚡ FAST SYNC: {} blk/s | Processed: {} | Received: {} | Requested: {} | Pending: {} | Queued: {} | Orphans: {}",
                      blocks_per_sec.round(),
                      processed,
                      received,
                      requested,
                      pending,
                      queued,
                      orphans);
            }
        });
    }
    
    async fn start_timeout_checker(&self) {
        let pending_requests = Arc::clone(&self.pending_requests);
        let blocks_to_download = Arc::clone(&self.blocks_to_download);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                let mut timed_out = Vec::new();
                {
                    let pending = pending_requests.lock().await;
                    let now = Instant::now();
                    
                    for (hash, request) in pending.iter() {
                        if now.duration_since(request.requested_at).as_secs() > REQUEST_TIMEOUT_SECS {
                            timed_out.push((*hash, request.clone()));
                        }
                    }
                }
                
                if !timed_out.is_empty() {
                    let mut pending = pending_requests.lock().await;
                    let mut queue = blocks_to_download.lock().await;
                    
                    for (hash, mut request) in timed_out {
                        pending.remove(&hash);
                        
                        if request.retry_count < MAX_RETRIES {
                            request.retry_count += 1;
                            queue.push_back(hash);
                            warn!("Request for block {} timed out, retrying (attempt {})", 
                                  hex::encode(&hash[..8]), request.retry_count + 1);
                        } else {
                            error!("Block {} failed after {} retries, giving up", 
                                   hex::encode(&hash[..8]), MAX_RETRIES);
                        }
                    }
                }
            }
        });
    }
    
    pub async fn get_stats(&self) -> FastSyncStats {
        let requested = *self.total_requested.lock().await;
        let received = *self.total_received.lock().await;
        let processed = *self.total_processed.lock().await;
        let pending = self.pending_requests.lock().await.len();
        let queued = self.downloaded_blocks.lock().await.len();
        let orphans = self.orphan_manager.get_orphan_count().await;
        
        let elapsed = {
            let start = self.start_time.lock().await;
            start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0)
        };
        
        let blocks_per_sec = if elapsed > 0.0 {
            processed as f64 / elapsed
        } else {
            0.0
        };
        
        FastSyncStats {
            total_requested: requested,
            total_received: received,
            total_processed: processed,
            pending_requests: pending as u64,
            queued_blocks: queued as u64,
            orphan_blocks: orphans as u64,
            blocks_per_second: blocks_per_sec,
            elapsed_seconds: elapsed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FastSyncStats {
    pub total_requested: u64,
    pub total_received: u64,
    pub total_processed: u64,
    pub pending_requests: u64,
    pub queued_blocks: u64,
    pub orphan_blocks: u64,
    pub blocks_per_second: f64,
    pub elapsed_seconds: f64,
} 