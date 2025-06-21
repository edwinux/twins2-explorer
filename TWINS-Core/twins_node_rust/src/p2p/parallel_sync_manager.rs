// TWINS-Core/twins_node_rust/src/p2p/parallel_sync_manager.rs
// Multi-threaded, torrent-like sync manager for downloading blocks in parallel from multiple peers

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{RwLock as TokioRwLock, Mutex as TokioMutex};
use tokio::time::{interval, Duration, Instant};
use log::{info, warn, error, debug};

use crate::blockchain::chain_state::ChainState;
use crate::storage::BlockStorage;
use crate::p2p::messages::{BlockMessage, GetDataMessage, InventoryVector, InventoryType};

// Configuration constants for torrent-like parallel sync
const MAX_PARALLEL_DOWNLOADS: usize = 16; // Maximum concurrent downloads (increased for torrent-like behavior)
const SEGMENT_SIZE: u32 = 500;             // Larger segments for fewer network requests
const MAX_SEGMENTS_PER_PEER: usize = 4;    // Max segments downloading from one peer
const DOWNLOAD_TIMEOUT_SECONDS: u64 = 20;  // Shorter timeout for faster recovery
const MIN_PEERS_FOR_PARALLEL: usize = 2;   // Minimum peers needed to start parallel sync

#[derive(Debug, Clone)]
pub struct BlockSegment {
    pub start_height: u32,
    pub end_height: u32,
    pub blocks: Vec<Option<BlockMessage>>, // None = not downloaded yet
    pub assigned_peer: Option<std::net::SocketAddr>,
    pub download_started: Option<Instant>,
    pub retry_count: u32,
}

impl BlockSegment {
    pub fn new(start_height: u32, end_height: u32) -> Self {
        let size = (end_height - start_height + 1) as usize;
        Self {
            start_height,
            end_height,
            blocks: vec![None; size],
            assigned_peer: None,
            download_started: None,
            retry_count: 0,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.blocks.iter().all(|block| block.is_some())
    }

    pub fn completion_percentage(&self) -> f64 {
        let completed = self.blocks.iter().filter(|block| block.is_some()).count();
        (completed as f64 / self.blocks.len() as f64) * 100.0
    }

    pub fn set_block(&mut self, height: u32, block: BlockMessage) -> bool {
        if height >= self.start_height && height <= self.end_height {
            let index = (height - self.start_height) as usize;
            if index < self.blocks.len() {
                self.blocks[index] = Some(block);
                return true;
            }
        }
        false
    }

    pub fn get_missing_heights(&self) -> Vec<u32> {
        self.blocks
            .iter()
            .enumerate()
            .filter_map(|(i, block)| {
                if block.is_none() {
                    Some(self.start_height + i as u32)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct PeerDownloadState {
    pub active_segments: HashSet<u32>, // segment start heights
    pub last_activity: Instant,
    pub download_speed: f64, // blocks per second
    pub reliability_score: f64, // 0.0 to 1.0
}

impl PeerDownloadState {
    pub fn new() -> Self {
        Self {
            active_segments: HashSet::new(),
            last_activity: Instant::now(),
            download_speed: 0.0,
            reliability_score: 1.0,
        }
    }
}

pub struct ParallelSyncManager {
    chain_state: Arc<ChainState>,
    storage: Arc<dyn BlockStorage>,
    
    // Segment management
    segments: Arc<TokioRwLock<HashMap<u32, BlockSegment>>>, // key = start_height
    completed_segments: Arc<TokioMutex<VecDeque<u32>>>, // queue of completed segment start heights
    
    // Peer management
    peer_states: Arc<TokioRwLock<HashMap<std::net::SocketAddr, PeerDownloadState>>>,
    available_peers: Arc<TokioMutex<VecDeque<std::net::SocketAddr>>>,
    
    // Pipeline for processing completed segments
    processing_pipeline: Arc<TokioMutex<VecDeque<BlockMessage>>>,
    
    // Statistics
    total_segments: Arc<StdMutex<u32>>,
    completed_count: Arc<StdMutex<u32>>,
    sync_start_time: Arc<StdMutex<Option<Instant>>>,
    
    // Download tracking
    total_downloaded_blocks: Arc<StdMutex<u32>>, // Total blocks downloaded (including not yet synced)
    total_synced_blocks: Arc<StdMutex<u32>>,     // Total blocks processed to chain tip
    sync_range: Arc<StdMutex<(u32, u32)>>,      // (start_height, end_height) for positioning
    
    // Network access for sending requests
    request_sender: Arc<TokioMutex<Option<tokio::sync::mpsc::Sender<(std::net::SocketAddr, Vec<[u8; 32]>)>>>>,
}

impl ParallelSyncManager {
    pub fn new(
        chain_state: Arc<ChainState>,
        storage: Arc<dyn BlockStorage>,
    ) -> Self {
        Self {
            chain_state,
            storage,
            segments: Arc::new(TokioRwLock::new(HashMap::new())),
            completed_segments: Arc::new(TokioMutex::new(VecDeque::new())),
            peer_states: Arc::new(TokioRwLock::new(HashMap::new())),
            available_peers: Arc::new(TokioMutex::new(VecDeque::new())),
            processing_pipeline: Arc::new(TokioMutex::new(VecDeque::new())),
            total_segments: Arc::new(StdMutex::new(0)),
            completed_count: Arc::new(StdMutex::new(0)),
            sync_start_time: Arc::new(StdMutex::new(None)),
            total_downloaded_blocks: Arc::new(StdMutex::new(0)),
            total_synced_blocks: Arc::new(StdMutex::new(0)),
            sync_range: Arc::new(StdMutex::new((0, 0))),
            request_sender: Arc::new(TokioMutex::new(None)),
        }
    }

    pub async fn start_parallel_sync(&self, target_height: u32) {
        let current_height = self.chain_state.get_tip().map_or(0, |tip| tip.height);
        
        if current_height >= target_height {
            info!("Already synced to target height {}", target_height);
            return;
        }

        // Check if we have enough peers for parallel sync
        let peer_count = {
            let peers = self.peer_states.read().await;
            peers.len()
        };

        if peer_count < MIN_PEERS_FOR_PARALLEL {
            warn!("Only {} peers available, need at least {} for torrent-like sync", 
                  peer_count, MIN_PEERS_FOR_PARALLEL);
            return;
        }

        // Initialize sync start time
        {
            let mut start_time = self.sync_start_time.lock().unwrap();
            *start_time = Some(Instant::now());
        }

        let blocks_to_sync = target_height - current_height;
        let estimated_segments = (blocks_to_sync + SEGMENT_SIZE - 1) / SEGMENT_SIZE;

        info!("🌊 Starting TORRENT-LIKE sync from {} to {} ({} blocks in {} segments with {} peers)", 
              current_height, target_height, blocks_to_sync, estimated_segments, peer_count);

        // Store sync range for positioning
        {
            let mut sync_range = self.sync_range.lock().unwrap();
            *sync_range = (current_height + 1, target_height);
        }

        // Create segments
        self.create_segments(current_height + 1, target_height).await;

        // Start the download coordinator
        self.start_download_coordinator().await;

        // Start the processing pipeline
        self.start_processing_pipeline().await;

        // Start statistics reporter
        self.start_statistics_reporter().await;
    }

    async fn create_segments(&self, start_height: u32, end_height: u32) {
        let mut segments = self.segments.write().await;
        let mut total = 0;

        let mut current = start_height;
        while current <= end_height {
            let segment_end = std::cmp::min(current + SEGMENT_SIZE - 1, end_height);
            let segment = BlockSegment::new(current, segment_end);
            
            segments.insert(current, segment);
            total += 1;
            
            current = segment_end + 1;
        }

        {
            let mut total_segments = self.total_segments.lock().unwrap();
            *total_segments = total;
        }

        info!("📦 Created {} segments for parallel download", total);
    }

    async fn start_download_coordinator(&self) {
        let segments = Arc::clone(&self.segments);
        let peer_states = Arc::clone(&self.peer_states);
        let available_peers = Arc::clone(&self.available_peers);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));

            loop {
                interval.tick().await;

                let mut segments_guard = segments.write().await;
                let mut peer_states_guard = peer_states.write().await;
                let mut available_peers_guard = available_peers.lock().await;

                // Clean up timed out downloads
                Self::cleanup_timed_out_downloads(&mut segments_guard, &mut peer_states_guard).await;

                // Assign new downloads more aggressively
                let mut assignments = 0;
                let current_active = segments_guard.values()
                    .filter(|s| s.assigned_peer.is_some() && !s.is_complete())
                    .count();

                if current_active < MAX_PARALLEL_DOWNLOADS {
                    for (start_height, segment) in segments_guard.iter_mut() {
                        if segment.assigned_peer.is_none() && !segment.is_complete() {
                            if let Some(peer_addr) = Self::find_best_peer(&peer_states_guard, &mut available_peers_guard) {
                                segment.assigned_peer = Some(peer_addr);
                                segment.download_started = Some(Instant::now());
                                
                                if let Some(peer_state) = peer_states_guard.get_mut(&peer_addr) {
                                    peer_state.active_segments.insert(*start_height);
                                }

                                // Send GETDATA request for this segment
                                Self::send_segment_request(segment, peer_addr).await;

                                assignments += 1;
                                debug!("📥 Assigned segment {}-{} to peer {} (total active: {})", 
                                       segment.start_height, segment.end_height, peer_addr, current_active + assignments);

                                if assignments >= (MAX_PARALLEL_DOWNLOADS - current_active) {
                                    break;
                                }
                            }
                        }
                    }
                }

                if segments_guard.values().all(|s| s.is_complete()) {
                    break;
                }
            }

            info!("📋 Download coordinator finished");
        });
    }

    async fn cleanup_timed_out_downloads(
        segments: &mut HashMap<u32, BlockSegment>,
        peer_states: &mut HashMap<std::net::SocketAddr, PeerDownloadState>,
    ) {
        let timeout_duration = Duration::from_secs(DOWNLOAD_TIMEOUT_SECONDS);
        let now = Instant::now();

        for (start_height, segment) in segments.iter_mut() {
            if let Some(download_start) = segment.download_started {
                if now.duration_since(download_start) > timeout_duration {
                    warn!("⏰ Segment {} timed out, reassigning", start_height);
                    
                    if let Some(peer_addr) = segment.assigned_peer {
                        if let Some(peer_state) = peer_states.get_mut(&peer_addr) {
                            peer_state.active_segments.remove(start_height);
                            peer_state.reliability_score *= 0.8;
                        }
                    }

                    segment.assigned_peer = None;
                    segment.download_started = None;
                    segment.retry_count += 1;

                    if segment.retry_count > 3 {
                        error!("🚫 Segment {} failed too many times", start_height);
                    }
                }
            }
        }
    }

    fn find_best_peer(
        peer_states: &HashMap<std::net::SocketAddr, PeerDownloadState>,
        available_peers: &mut VecDeque<std::net::SocketAddr>,
    ) -> Option<std::net::SocketAddr> {
        let mut best_peer = None;
        let mut best_score = -1.0;

        for peer_addr in available_peers.iter() {
            if let Some(peer_state) = peer_states.get(peer_addr) {
                if peer_state.active_segments.len() < MAX_SEGMENTS_PER_PEER {
                    let load_factor = 1.0 - (peer_state.active_segments.len() as f64 / MAX_SEGMENTS_PER_PEER as f64);
                    let score = peer_state.reliability_score * load_factor + peer_state.download_speed * 0.1;
                    
                    if score > best_score {
                        best_score = score;
                        best_peer = Some(*peer_addr);
                    }
                }
            }
        }

        best_peer
    }

    async fn send_segment_request(segment: &BlockSegment, peer_addr: std::net::SocketAddr) {
        // Create inventory vectors for all blocks in this segment
        let mut inventory_items = Vec::new();
        
        for height in segment.start_height..=segment.end_height {
            // For now, we'll request by height. In a full implementation,
            // you'd need to convert height to block hash
            let inv_vector = InventoryVector {
                inv_type: InventoryType::Block,
                hash: [0u8; 32], // Placeholder - would need actual block hash
            };
            inventory_items.push(inv_vector);
        }

        if !inventory_items.is_empty() {
            // TODO: Send GETDATA message to peer
            // This would require access to the connection/peer manager
            debug!("📤 Would send GETDATA for {} blocks to {}", inventory_items.len(), peer_addr);
        }
    }

    async fn start_processing_pipeline(&self) {
        let processing_pipeline = Arc::clone(&self.processing_pipeline);
        let completed_segments = Arc::clone(&self.completed_segments);
        let segments = Arc::clone(&self.segments);
        let storage = Arc::clone(&self.storage);
        let chain_state = Arc::clone(&self.chain_state);
        let completed_count = Arc::clone(&self.completed_count);
        let total_synced_blocks = Arc::clone(&self.total_synced_blocks);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));

            loop {
                interval.tick().await;

                // Check for completed segments
                {
                    let mut completed_queue = completed_segments.lock().await;
                    let segments_guard = segments.read().await;

                    for (start_height, segment) in segments_guard.iter() {
                        if segment.is_complete() && !completed_queue.iter().any(|&h| h == *start_height) {
                            completed_queue.push_back(*start_height);
                            info!("✅ Segment {} completed ({:.1}%)", 
                                  start_height, segment.completion_percentage());
                        }
                    }
                }

                // Process completed segments
                {
                    let mut completed_queue = completed_segments.lock().await;
                    let segments_guard = segments.read().await;

                    while let Some(&start_height) = completed_queue.front() {
                        if let Some(segment) = segments_guard.get(&start_height) {
                            if segment.is_complete() {
                                let mut pipeline = processing_pipeline.lock().await;
                                for block_opt in &segment.blocks {
                                    if let Some(block) = block_opt {
                                        pipeline.push_back(block.clone());
                                    }
                                }

                                completed_queue.pop_front();
                                
                                let mut count = completed_count.lock().unwrap();
                                *count += 1;

                                debug!("📦 Added segment {} to processing pipeline", start_height);
                            } else {
                                break;
                            }
                        } else {
                            completed_queue.pop_front();
                        }
                    }
                }

                // Process blocks from pipeline
                {
                    let mut pipeline = processing_pipeline.lock().await;
                    let mut processed = 0;

                    while let Some(block) = pipeline.pop_front() {
                        let block_hash = block.header.get_hash();
                        
                        match storage.save_block(&block) {
                            Ok(_) => {
                                chain_state.add_block_index(block.header.clone());
                                processed += 1;
                                
                                // Update synced blocks counter
                                {
                                    let mut synced_blocks = total_synced_blocks.lock().unwrap();
                                    *synced_blocks += 1;
                                }
                                
                                if processed >= 50 {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("❌ Failed to store block {}: {}", hex::encode(block_hash), e);
                            }
                        }
                    }

                    if processed > 0 {
                        debug!("🔄 Processed {} blocks from pipeline", processed);
                    }
                }

                // Check if sync is complete
                {
                    let segments_guard = segments.read().await;
                    if segments_guard.values().all(|s| s.is_complete()) {
                        let pipeline = processing_pipeline.lock().await;
                        if pipeline.is_empty() {
                            info!("🎉 Parallel sync completed successfully!");
                            break;
                        }
                    }
                }
            }
        });
    }

    async fn start_statistics_reporter(&self) {
        let segments = Arc::clone(&self.segments);
        let completed_count = Arc::clone(&self.completed_count);
        let total_segments = Arc::clone(&self.total_segments);
        let sync_start_time = Arc::clone(&self.sync_start_time);
        let sync_range = Arc::clone(&self.sync_range);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                let completed = *completed_count.lock().unwrap();
                let total = *total_segments.lock().unwrap();
                
                if total == 0 {
                    continue;
                }

                let progress = (completed as f64 / total as f64) * 100.0;
                
                let elapsed = {
                    let start_time = sync_start_time.lock().unwrap();
                    start_time.map(|t| t.elapsed().as_secs_f64()).unwrap_or(1.0)
                };

                let segments_per_second = completed as f64 / elapsed;
                let blocks_per_second = segments_per_second * SEGMENT_SIZE as f64;

                let segments_guard = segments.read().await;
                let active_downloads = segments_guard.values()
                    .filter(|s| s.assigned_peer.is_some() && !s.is_complete())
                    .count();

                // Create torrent-like visual progress bar
                let sync_range_val = {
                    let range = sync_range.lock().unwrap();
                    *range
                };
                let visual_progress = Self::create_visual_progress(&segments_guard, sync_range_val);

                info!("🌊 TORRENT SYNC: {:.1}% [{:60}] {:.0} blk/s ⬇{} active", 
                      progress, visual_progress, blocks_per_second, active_downloads);

                // Detailed segment status every 30 seconds
                if elapsed as u64 % 30 < 5 {
                    Self::print_detailed_segment_status(&segments_guard).await;
                }

                if completed >= total {
                    info!("🎉 TORRENT SYNC COMPLETED! {} segments downloaded in {:.1}s", total, elapsed);
                    break;
                }
            }
        });
    }

    fn create_visual_progress(segments: &HashMap<u32, BlockSegment>, sync_range: (u32, u32)) -> String {
        const PROGRESS_WIDTH: usize = 60;
        let mut progress_chars = vec!['·'; PROGRESS_WIDTH];
        
        let (start_height, end_height) = sync_range;
        if start_height >= end_height {
            return "·".repeat(PROGRESS_WIDTH);
        }

        let total_blocks = end_height - start_height + 1;
        let blocks_per_char = (total_blocks as f64 / PROGRESS_WIDTH as f64).max(1.0);
        
        for (_, segment) in segments.iter() {
            // Calculate the position of this segment in the progress bar
            let segment_start_offset = segment.start_height.saturating_sub(start_height);
            let segment_end_offset = segment.end_height.saturating_sub(start_height);
            
            let start_char = ((segment_start_offset as f64 / blocks_per_char) as usize).min(PROGRESS_WIDTH - 1);
            let end_char = ((segment_end_offset as f64 / blocks_per_char) as usize).min(PROGRESS_WIDTH - 1);
            
            // Determine the character to use for this segment
            let segment_char = if segment.is_complete() {
                '█' // Completed
            } else if segment.assigned_peer.is_some() {
                let completion = segment.completion_percentage();
                if completion >= 75.0 {
                    '▓' // Almost done
                } else if completion >= 50.0 {
                    '▒' // Half done
                } else if completion >= 25.0 {
                    '░' // Started
                } else {
                    '▁' // Just started
                }
            } else {
                '·' // Waiting
            };
            
            // Fill the range for this segment
            for char_index in start_char..=end_char {
                if char_index < PROGRESS_WIDTH {
                    // Only overwrite if the new state is "better" than waiting
                    if progress_chars[char_index] == '·' || segment_char != '·' {
                        progress_chars[char_index] = segment_char;
                    }
                }
            }
        }
        
        progress_chars.iter().collect()
    }

    async fn print_detailed_segment_status(segments: &HashMap<u32, BlockSegment>) {
        let mut completed = 0;
        let mut downloading = 0;
        let mut waiting = 0;
        let mut failed = 0;

        for segment in segments.values() {
            if segment.is_complete() {
                completed += 1;
            } else if segment.assigned_peer.is_some() {
                downloading += 1;
            } else if segment.retry_count > 0 {
                failed += 1;
            } else {
                waiting += 1;
            }
        }

        info!("📋 SEGMENT STATUS: ✅ {} completed | ⬇ {} downloading | ⏳ {} waiting | ❌ {} failed", 
              completed, downloading, waiting, failed);

        // Show top downloading segments
        let mut downloading_segments: Vec<_> = segments.iter()
            .filter(|(_, s)| s.assigned_peer.is_some() && !s.is_complete())
            .collect();
        downloading_segments.sort_by_key(|(start_height, _)| *start_height);

        if !downloading_segments.is_empty() {
            let active_list: Vec<String> = downloading_segments.iter()
                .take(5)
                .map(|(start_height, segment)| {
                    format!("{}({:.0}%)", start_height, segment.completion_percentage())
                })
                .collect();
            info!("🔄 ACTIVE SEGMENTS: {}", active_list.join(", "));
        }
    }

    pub async fn add_peer(&self, peer_addr: std::net::SocketAddr) {
        {
            let mut peer_states = self.peer_states.write().await;
            peer_states.insert(peer_addr, PeerDownloadState::new());
        }

        {
            let mut available_peers = self.available_peers.lock().await;
            available_peers.push_back(peer_addr);
        }

        info!("🔗 Added peer {} to parallel sync pool", peer_addr);
    }

    pub async fn remove_peer(&self, peer_addr: std::net::SocketAddr) {
        {
            let mut peer_states = self.peer_states.write().await;
            peer_states.remove(&peer_addr);
        }

        {
            let mut available_peers = self.available_peers.lock().await;
            available_peers.retain(|&addr| addr != peer_addr);
        }

        {
            let mut segments = self.segments.write().await;
            for segment in segments.values_mut() {
                if segment.assigned_peer == Some(peer_addr) {
                    segment.assigned_peer = None;
                    segment.download_started = None;
                }
            }
        }

        info!("🔌 Removed peer {} from parallel sync pool", peer_addr);
    }

    pub async fn handle_block_received(&self, block: BlockMessage, from_peer: std::net::SocketAddr) {
        // Update downloaded blocks counter
        {
            let mut downloaded_blocks = self.total_downloaded_blocks.lock().unwrap();
            *downloaded_blocks += 1;
        }
        
        // Check if parallel sync is active
        let total_segments = *self.total_segments.lock().unwrap();
        if total_segments == 0 {
            // Regular sync mode - just track metrics
            debug!("📦 Tracked block download from {} (total downloaded: {})", 
                   from_peer, *self.total_downloaded_blocks.lock().unwrap());
            return;
        }
        
        // Parallel sync mode - manage segments
        let block_height = self.get_block_height(&block).await;
        let mut segments = self.segments.write().await;
        
        for (start_height, segment) in segments.iter_mut() {
            if block_height >= segment.start_height && block_height <= segment.end_height {
                if segment.set_block(block_height, block.clone()) {
                    debug!("📦 Received block {} for segment {} from {}", 
                           block_height, start_height, from_peer);

                    {
                        let mut peer_states = self.peer_states.write().await;
                        if let Some(peer_state) = peer_states.get_mut(&from_peer) {
                            peer_state.last_activity = Instant::now();
                            peer_state.reliability_score = (peer_state.reliability_score * 0.9 + 0.1).min(1.0);
                        }
                    }

                    if segment.is_complete() {
                        info!("✅ Segment {} completed by peer {}", start_height, from_peer);
                        
                        {
                            let mut peer_states = self.peer_states.write().await;
                            if let Some(peer_state) = peer_states.get_mut(&from_peer) {
                                peer_state.active_segments.remove(start_height);
                            }
                        }
                    }
                    
                    break;
                }
            }
        }
    }

    pub async fn handle_block_synced(&self) {
        // Track when a block has been successfully synced to the chain
        {
            let mut synced_blocks = self.total_synced_blocks.lock().unwrap();
            *synced_blocks += 1;
        }
        
        debug!("🔗 Tracked block sync (total synced: {})", 
               *self.total_synced_blocks.lock().unwrap());
    }

    async fn get_block_height(&self, block: &BlockMessage) -> u32 {
        let prev_hash = block.header.prev_block_hash;
        if let Ok(Some(prev_header)) = self.storage.get_header(&prev_hash) {
            return prev_header.height + 1;
        }
        
        self.chain_state.get_tip().map_or(0, |tip| tip.height + 1)
    }

    pub async fn get_sync_progress(&self) -> (u32, u32, f64) {
        let completed = *self.completed_count.lock().unwrap();
        let total = *self.total_segments.lock().unwrap();
        let progress = if total > 0 { (completed as f64 / total as f64) * 100.0 } else { 0.0 };
        
        (completed, total, progress)
    }

    pub async fn get_detailed_sync_status(&self) -> TorrentSyncStatus {
        let segments = self.segments.read().await;
        let total_segments = *self.total_segments.lock().unwrap();
        
        // For regular sync mode (when parallel sync is not active)
        if total_segments == 0 {
            let total_downloaded_blocks = *self.total_downloaded_blocks.lock().unwrap();
            let total_synced_blocks = *self.total_synced_blocks.lock().unwrap();
            
            return TorrentSyncStatus {
                is_active: false, // Regular sync, not torrent-like
                total_segments: 0,
                completed_segments: 0,
                downloading_segments: 0,
                waiting_segments: 0,
                failed_segments: 0,
                blocks_per_second: 0.0,
                visual_progress: "·".repeat(60),
                active_downloads: vec![],
                elapsed_seconds: 0.0,
                total_downloaded_blocks,
                total_synced_blocks,
                sync_range: (0, 0),
            };
        }

        let mut completed = 0;
        let mut downloading = 0;
        let mut waiting = 0;
        let mut failed = 0;
        let mut active_downloads = Vec::new();

        for (_, segment) in segments.iter() {
            if segment.is_complete() {
                completed += 1;
            } else if segment.assigned_peer.is_some() {
                downloading += 1;
                active_downloads.push(TorrentSegmentInfo {
                    start_height: segment.start_height,
                    end_height: segment.end_height,
                    completion_percentage: segment.completion_percentage(),
                    peer_addr: segment.assigned_peer.map(|addr| addr.to_string()),
                });
            } else if segment.retry_count > 0 {
                failed += 1;
            } else {
                waiting += 1;
            }
        }

        // Calculate speed
        let elapsed = {
            let start_time = self.sync_start_time.lock().unwrap();
            start_time.map(|t| t.elapsed().as_secs_f64()).unwrap_or(1.0)
        };
        let blocks_per_second = (completed as f64 * SEGMENT_SIZE as f64) / elapsed;

        // Get additional metrics
        let total_downloaded_blocks = *self.total_downloaded_blocks.lock().unwrap();
        let total_synced_blocks = *self.total_synced_blocks.lock().unwrap();
        let sync_range = *self.sync_range.lock().unwrap();

        // Create visual progress
        let visual_progress = Self::create_visual_progress(&segments, sync_range);

        TorrentSyncStatus {
            is_active: downloading > 0 || waiting > 0,
            total_segments,
            completed_segments: completed,
            downloading_segments: downloading,
            waiting_segments: waiting,
            failed_segments: failed,
            blocks_per_second,
            visual_progress,
            active_downloads,
            elapsed_seconds: elapsed,
            total_downloaded_blocks,
            total_synced_blocks,
            sync_range,
        }
    }
}

#[derive(Debug)]
pub struct TorrentSyncStatus {
    pub is_active: bool,
    pub total_segments: u32,
    pub completed_segments: u32,
    pub downloading_segments: u32,
    pub waiting_segments: u32,
    pub failed_segments: u32,
    pub blocks_per_second: f64,
    pub visual_progress: String,
    pub active_downloads: Vec<TorrentSegmentInfo>,
    pub elapsed_seconds: f64,
    pub total_downloaded_blocks: u32,
    pub total_synced_blocks: u32,
    pub sync_range: (u32, u32),
}

#[derive(Debug)]
pub struct TorrentSegmentInfo {
    pub start_height: u32,
    pub end_height: u32,
    pub completion_percentage: f64,
    pub peer_addr: Option<String>,
}
