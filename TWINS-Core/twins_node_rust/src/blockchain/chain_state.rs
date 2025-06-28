use crate::blockchain::block_index::BlockIndex;
use crate::p2p::messages::BlockHeaderData;
use std::collections::HashMap;
use std::sync::{Arc, RwLock}; // Using RwLock for concurrent reads and one-writer access
use hex;
use crate::storage::BlockStorage; // Added storage import

// Helper function to create the mainnet genesis BlockIndex
pub fn get_mainnet_genesis_block_index() -> BlockIndex {
    let mut merkle_root_bytes = [0u8; 32];
    hex::decode_to_slice("4271a3d993d6157f960de646ce8dfad07989dfd0703064f8056d1a7287283d06", &mut merkle_root_bytes)
        .expect("Failed to decode genesis merkle root");

    let genesis_header_data = BlockHeaderData {
        version: 1,
        prev_block_hash: [0u8; 32], // Genesis prev_block_hash is null
        merkle_root: merkle_root_bytes,
        timestamp: 1546790318,
        bits: 0x1e0ffff0,
        nonce: 348223,
        accumulator_checkpoint: None, // Version 1 block does not have this
    };

    BlockIndex::new(genesis_header_data, 0, None)
}

// #[derive(Debug)] // Removed derive Debug
pub struct ChainState {
    // All known block indexes, including forks
    block_index_map: RwLock<HashMap<[u8; 32], Arc<BlockIndex>>>,
    // Pointer to the current best chain tip
    current_tip: RwLock<Option<Arc<BlockIndex>>>,
    // Genesis block hash, set on initialization
    pub genesis_hash: [u8; 32], // Made public for API service access
    storage: Arc<dyn BlockStorage>, // Added storage field
}

// Manual Debug implementation for ChainState
impl std::fmt::Debug for ChainState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChainState")
         .field("block_index_map_len", &self.block_index_map.read().unwrap().len()) // Example: show length
         .field("current_tip_height", &self.current_tip.read().unwrap().as_ref().map_or(0, |t| t.height))
         .field("genesis_hash", &hex::encode(self.genesis_hash))
         .field("storage", &"Arc<dyn BlockStorage>") // Placeholder for non-Debug field
         .finish()
    }
}

impl ChainState {
    pub fn new(storage: Arc<dyn BlockStorage>) -> Self { 
        let genesis_block_index_obj = get_mainnet_genesis_block_index();
        let genesis_hash = genesis_block_index_obj.hash;
        let genesis_arc = Arc::new(genesis_block_index_obj);

        let mut map = HashMap::new();
        let initial_tip_arc: Arc<BlockIndex>;

        match storage.get_chain_tip_hash() {
            Ok(Some(tip_hash_from_db)) => {
                log::info!("Found existing chain tip hash in DB: {}", hex::encode(tip_hash_from_db));
                match storage.get_header(&tip_hash_from_db) {
                    Ok(Some(loaded_tip_index)) => {
                        log::info!("Successfully loaded tip BlockIndex from DB: height={}", loaded_tip_index.height);
                        initial_tip_arc = Arc::new(loaded_tip_index);
                        map.insert(tip_hash_from_db, Arc::clone(&initial_tip_arc));
                        // Ensure genesis is also in map if tip isn't genesis and not already loaded
                        if tip_hash_from_db != genesis_hash && !map.contains_key(&genesis_hash) {
                            match storage.get_header(&genesis_hash) {
                                Ok(Some(loaded_genesis_idx)) => {
                                    map.insert(genesis_hash, Arc::new(loaded_genesis_idx));
                                }
                                _ => { // Genesis not in DB, but tip is far along. This is unusual.
                                    log::warn!("Genesis block not found in DB while loading a non-genesis tip. Adding genesis to map and DB.");
                                    if let Err(e) = storage.save_header(&genesis_arc) {
                                        log::error!("Failed to save genesis header to DB during tip load: {}", e);
                                    }
                                    map.insert(genesis_hash, Arc::clone(&genesis_arc));
                                }
                            }
                        }
                    }
                    _ => { // Failed to load tip header, or it wasn't found despite hash being there
                        log::warn!("Chain tip hash found in DB, but failed to load tip BlockIndex. Re-initializing with genesis.");
                        if let Err(e) = storage.save_header(&genesis_arc) { log::error!("Failed to save genesis header (on tip load failure): {}", e); }
                        if let Err(e) = storage.set_chain_tip_hash(&genesis_hash) { log::error!("Failed to set genesis as tip hash (on tip load failure): {}", e); }
                        map.insert(genesis_hash, Arc::clone(&genesis_arc));
                        initial_tip_arc = Arc::clone(&genesis_arc);
                    }
                }
            }
            _ => { // No tip hash found, or error reading it. Initialize with genesis.
                log::info!("No chain tip hash found in DB, or error reading. Initializing with genesis block.");
                if let Err(e) = storage.save_header(&genesis_arc) { log::error!("Failed to save genesis header (on initial setup): {}", e); }
                if let Err(e) = storage.set_chain_tip_hash(&genesis_hash) { log::error!("Failed to set genesis as tip hash (on initial setup): {}", e); }
                map.insert(genesis_hash, Arc::clone(&genesis_arc));
                initial_tip_arc = Arc::clone(&genesis_arc);
            }
        }

        ChainState {
            block_index_map: RwLock::new(map),
            current_tip: RwLock::new(Some(initial_tip_arc)),
            genesis_hash,
            storage,
        }
    }

    // Adds a block index if it's not already present.
    // Potentially updates the chain tip if this new block extends the current best chain
    // and has more cumulative work (simplified to height for now).
    // Returns true if the block was added (and potentially became tip), false if already known or orphaned (for now).
    pub fn add_block_index(&self, header_data: crate::p2p::messages::BlockHeaderData) -> Option<Arc<BlockIndex>> {
        let new_hash = header_data.get_hash();
        
        // Check if already in memory map first
        {
            let map_read = self.block_index_map.read().unwrap();
            if let Some(existing_index) = map_read.get(&new_hash) {
                return Some(Arc::clone(existing_index));
            }
        }

        let prev_hash_opt = if header_data.prev_block_hash == [0u8; 32] { None } else { Some(header_data.prev_block_hash) };
        
        let prev_block_index_arc_opt: Option<Arc<BlockIndex>> = match prev_hash_opt {
            Some(ph) => {
                let map_read = self.block_index_map.read().unwrap();
                map_read.get(&ph).cloned().or_else(|| {
                    drop(map_read); // Release read lock before potential write lock in self.storage.get_header if it internally uses map
                    match self.storage.get_header(&ph) {
                        Ok(Some(bi_from_db)) => {
                            let bi_arc = Arc::new(bi_from_db);
                            // Add to in-memory map if loaded from DB
                            let mut map_write = self.block_index_map.write().unwrap();
                            map_write.insert(ph, Arc::clone(&bi_arc));
                            Some(bi_arc)
                        }
                        _ => None,
                    }
                })
            }
            None => None, 
        };

        let height = match prev_block_index_arc_opt {
            Some(ref prev_bi) => prev_bi.height + 1,
            None => {
                if header_data.prev_block_hash == [0u8; 32] && new_hash == self.genesis_hash {
                    0
                } else {
                    // Temporary workaround: estimate height for orphan blocks
                    let current_tip_height = self.current_tip.read().unwrap().as_ref().map_or(0, |t| t.height);
                    let estimated_height = current_tip_height + 1;
                    log::warn!("Orphan block {} with unknown parent {} - adding with estimated height {}", 
                              hex::encode(new_hash), hex::encode(header_data.prev_block_hash), estimated_height);
                    estimated_height
                }
            }
        };

        let new_block_index = Arc::new(BlockIndex::new(header_data, height, prev_hash_opt));
        
        if let Err(e) = self.storage.save_header(&new_block_index) {
            log::error!("Failed to save header {} to DB: {}", hex::encode(new_hash), e);
            return None; 
        }
        log::debug!("Saved header {} to DB.", hex::encode(new_hash));

        {
            let mut map_write = self.block_index_map.write().unwrap();
            map_write.insert(new_hash, Arc::clone(&new_block_index));
        }
        log::debug!("Added block index to map: height={}, hash={}", height, hex::encode(new_hash));

        let mut tip_write = self.current_tip.write().unwrap();
        let mut tip_updated = false;
        match *tip_write {
            Some(ref current_tip_bi) => {
                if new_block_index.chain_work_proxy > current_tip_bi.chain_work_proxy {
                    *tip_write = Some(Arc::clone(&new_block_index));
                    tip_updated = true;
                }
            }
            None => { 
                *tip_write = Some(Arc::clone(&new_block_index));
                tip_updated = true;
            }
        }
        if tip_updated {
            if let Err(e) = self.storage.set_chain_tip_hash(&new_hash) {
                log::error!("Failed to set new chain tip hash {} in DB: {}", hex::encode(new_hash), e);
            }
            log::info!("New chain tip: height={}, hash={}", new_block_index.height, hex::encode(new_hash));
        }
        Some(new_block_index)
    }

    pub fn get_tip(&self) -> Option<Arc<BlockIndex>> {
        self.current_tip.read().unwrap().clone()
    }

    // Generates a block locator for getheaders message
    pub fn get_block_locator(&self) -> Vec<[u8; 32]> {
        let mut locator = Vec::new();
        let mut current_index_opt: Option<Arc<BlockIndex>> = self.current_tip.read().unwrap().clone();

        // Helper to get BlockIndex, trying memory then storage
        let get_bi = |hash_opt: Option<[u8;32]>, block_map_guard: &HashMap<[u8;32], Arc<BlockIndex>>, storage: &Arc<dyn BlockStorage>| -> Option<Arc<BlockIndex>> {
            hash_opt.and_then(|h| {
                block_map_guard.get(&h).cloned().or_else(|| {
                    storage.get_header(&h).ok().flatten().map(Arc::new)
                })
            })
        };

        let map_read_guard = self.block_index_map.read().unwrap();

        for _ in 0..10 {
            if let Some(ref current_index_arc) = current_index_opt {
                locator.push(current_index_arc.hash);
                current_index_opt = get_bi(current_index_arc.prev_hash, &map_read_guard, &self.storage);
                if current_index_opt.is_none() { break; }
            } else {
                break;
            }
        }

        let mut step = 1u32;
        while let Some(ref current_index_arc_loop2) = current_index_opt {
            if locator.len() >= 30 { break; }
            let mut temp_stepper_opt = Some(Arc::clone(current_index_arc_loop2));
            for _ in 0..step {
                if let Some(ref temp_stepper_arc) = temp_stepper_opt {
                    temp_stepper_opt = get_bi(temp_stepper_arc.prev_hash, &map_read_guard, &self.storage);
                    if temp_stepper_opt.is_none() { break; }
                } else {
                    temp_stepper_opt = None;
                    break;
                }
            }
            if let Some(found_ancestor_arc) = temp_stepper_opt {
                locator.push(found_ancestor_arc.hash);
                current_index_opt = Some(Arc::clone(&found_ancestor_arc));
            } else {
                current_index_opt = None;
                break;
            }
            if step.checked_mul(2).is_none() { break; }
            step *= 2;
        }
        drop(map_read_guard); // Release lock
        
        if !locator.contains(&self.genesis_hash) && locator.len() < 32 {
            // Check storage for genesis if not in current locator path and map already checked by get_bi
            if self.storage.get_header(&self.genesis_hash).ok().flatten().is_some() || self.block_index_map.read().unwrap().contains_key(&self.genesis_hash) {
                 locator.push(self.genesis_hash);
             }
        } else if locator.is_empty() && (self.storage.get_header(&self.genesis_hash).ok().flatten().is_some() || self.block_index_map.read().unwrap().contains_key(&self.genesis_hash)) {
             locator.push(self.genesis_hash);
        }
        locator
    }

    pub fn reset(&self) {
        if let Err(e) = self.storage.reset() {
            log::error!("Failed to reset storage: {}", e);
            return;
        }

        let genesis = get_mainnet_genesis_block_index();
        let genesis_arc = Arc::new(genesis);

        {
            let mut map = self.block_index_map.write().unwrap();
            map.clear();
            map.insert(self.genesis_hash, Arc::clone(&genesis_arc));
        }

        if let Err(e) = self.storage.save_header(&genesis_arc) {
            log::error!("Failed to save genesis header after reset: {}", e);
        }
        if let Err(e) = self.storage.set_chain_tip_hash(&self.genesis_hash) {
            log::error!("Failed to set tip hash after reset: {}", e);
        }

        let mut tip = self.current_tip.write().unwrap();
        *tip = Some(genesis_arc);
    }
}
