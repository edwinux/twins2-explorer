use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use crate::p2p::messages::BlockMessage;
use crate::blockchain::chain_state::ChainState;
use crate::storage::BlockStorage;
use log::{info, debug, warn};

const MAX_ORPHAN_BLOCKS: usize = 10000;
const ORPHAN_CLEANUP_INTERVAL_SECS: u64 = 60;

pub struct OrphanBlock {
    pub block: BlockMessage,
    pub received_time: std::time::Instant,
}

pub struct OrphanManager {
    // Map from block hash to orphan block
    orphans: Arc<TokioMutex<HashMap<[u8; 32], OrphanBlock>>>,
    // Map from parent hash to child hashes waiting for this parent
    orphan_map: Arc<TokioMutex<HashMap<[u8; 32], Vec<[u8; 32]>>>>,
    // Queue of blocks ready to be processed
    process_queue: Arc<TokioMutex<VecDeque<BlockMessage>>>,
    
    chain_state: Arc<ChainState>,
    storage: Arc<dyn BlockStorage>,
}

impl OrphanManager {
    pub fn new(
        chain_state: Arc<ChainState>,
        storage: Arc<dyn BlockStorage>,
    ) -> Self {
        let manager = Self {
            orphans: Arc::new(TokioMutex::new(HashMap::new())),
            orphan_map: Arc::new(TokioMutex::new(HashMap::new())),
            process_queue: Arc::new(TokioMutex::new(VecDeque::new())),
            chain_state,
            storage,
        };
        
        // Start cleanup task
        let orphans_clone = Arc::clone(&manager.orphans);
        let orphan_map_clone = Arc::clone(&manager.orphan_map);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(ORPHAN_CLEANUP_INTERVAL_SECS));
            loop {
                interval.tick().await;
                Self::cleanup_old_orphans(orphans_clone.clone(), orphan_map_clone.clone()).await;
            }
        });
        
        manager
    }
    
    pub async fn add_block(&self, block: BlockMessage) -> Result<(), String> {
        let block_hash = block.header.get_hash();
        let parent_hash = block.header.prev_block_hash;
        
        // Check if we already have this block
        if self.storage.has_block(&block_hash).unwrap_or(false) {
            debug!("Block {} already in storage, skipping", hex::encode(&block_hash[..8]));
            return Ok(());
        }
        
        // Check if parent exists in storage
        let parent_exists = self.storage.has_block(&parent_hash).unwrap_or(false) ||
                           self.storage.get_header(&parent_hash).unwrap_or(None).is_some();
        
        if parent_exists {
            // Parent exists, process this block immediately
            self.process_block(block).await?;
            
            // Check if any orphans were waiting for this block
            self.process_orphan_descendants(block_hash).await?;
        } else {
            // Parent doesn't exist, add to orphans
            self.add_orphan(block).await?;
        }
        
        Ok(())
    }
    
    async fn add_orphan(&self, block: BlockMessage) -> Result<(), String> {
        let block_hash = block.header.get_hash();
        let parent_hash = block.header.prev_block_hash;
        
        let mut orphans = self.orphans.lock().await;
        let mut orphan_map = self.orphan_map.lock().await;
        
        // Check size limit
        if orphans.len() >= MAX_ORPHAN_BLOCKS {
            return Err("Orphan pool is full".to_string());
        }
        
        // Add to orphans
        orphans.insert(block_hash, OrphanBlock {
            block,
            received_time: std::time::Instant::now(),
        });
        
        // Add to orphan map
        orphan_map.entry(parent_hash)
            .or_insert_with(Vec::new)
            .push(block_hash);
        
        debug!("Added orphan block {} waiting for parent {}", 
               hex::encode(&block_hash[..8]), hex::encode(&parent_hash[..8]));
        
        Ok(())
    }
    
    async fn process_block(&self, block: BlockMessage) -> Result<(), String> {
        let block_hash = block.header.get_hash();
        
        // Verify PoS signature if needed
        if block.is_likely_proof_of_stake() {
            match block.verify_pos_signature() {
                Ok(is_valid) => {
                    if !is_valid {
                        return Err(format!("PoS signature verification failed for block {}", hex::encode(&block_hash[..8])));
                    }
                }
                Err(e) => {
                    return Err(format!("PoS signature verification error: {}", e));
                }
            }
        }
        
        // Save block to storage
        self.storage.save_block(&block)
            .map_err(|e| format!("Failed to save block: {}", e))?;
        
        // Add to chain state
        if let Some(block_index) = self.chain_state.add_block_index(block.header.clone()) {
            info!("✅ Processed block {} at height {}", 
                  hex::encode(&block_hash[..8]), block_index.height);
        }
        
        Ok(())
    }
    
    async fn process_orphan_descendants(&self, parent_hash: [u8; 32]) -> Result<(), String> {
        let mut blocks_to_process = VecDeque::new();
        blocks_to_process.push_back(parent_hash);
        
        while let Some(current_hash) = blocks_to_process.pop_front() {
            // Get orphans waiting for this block
            let waiting_orphans = {
                let mut orphan_map = self.orphan_map.lock().await;
                orphan_map.remove(&current_hash).unwrap_or_default()
            };
            
            for orphan_hash in waiting_orphans {
                // Get the orphan block
                let orphan_block = {
                    let mut orphans = self.orphans.lock().await;
                    orphans.remove(&orphan_hash)
                };
                
                if let Some(orphan) = orphan_block {
                    // Process the orphan
                    self.process_block(orphan.block.clone()).await?;
                    
                    // Add this block's hash to check for more descendants
                    blocks_to_process.push_back(orphan_hash);
                    
                    debug!("Processed orphan {} after parent {} became available", 
                           hex::encode(&orphan_hash[..8]), hex::encode(&current_hash[..8]));
                }
            }
        }
        
        Ok(())
    }
    
    async fn cleanup_old_orphans(
        orphans: Arc<TokioMutex<HashMap<[u8; 32], OrphanBlock>>>,
        orphan_map: Arc<TokioMutex<HashMap<[u8; 32], Vec<[u8; 32]>>>>,
    ) {
        let mut orphans_guard = orphans.lock().await;
        let mut orphan_map_guard = orphan_map.lock().await;
        
        let now = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(300); // 5 minutes
        
        let mut to_remove = Vec::new();
        for (hash, orphan) in orphans_guard.iter() {
            if now.duration_since(orphan.received_time) > timeout {
                to_remove.push(*hash);
            }
        }
        
        for hash in to_remove {
            if let Some(orphan) = orphans_guard.remove(&hash) {
                // Remove from orphan map
                let parent_hash = orphan.block.header.prev_block_hash;
                if let Some(children) = orphan_map_guard.get_mut(&parent_hash) {
                    children.retain(|h| h != &hash);
                    if children.is_empty() {
                        orphan_map_guard.remove(&parent_hash);
                    }
                }
                
                warn!("Removed old orphan block {} (age: {:?})", 
                      hex::encode(&hash[..8]), now.duration_since(orphan.received_time));
            }
        }
    }
    
    pub async fn get_orphan_count(&self) -> usize {
        self.orphans.lock().await.len()
    }
    
    pub async fn get_orphan_info(&self) -> Vec<(String, String, u64)> {
        let orphans = self.orphans.lock().await;
        orphans.iter()
            .map(|(hash, orphan)| {
                (
                    hex::encode(&hash[..8]),
                    hex::encode(&orphan.block.header.prev_block_hash[..8]),
                    orphan.received_time.elapsed().as_secs(),
                )
            })
            .collect()
    }
} 