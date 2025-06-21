// TWINS-Core/twins_node_rust/src/blockchain/block_index.rs

use crate::p2p::messages::BlockHeaderData;
// For chain_work, we might need a u256-like type. For now, let's use [u8; 32] or u64 for simplicity
// if PoS chain work is just about height or a simpler metric.
// PIVX CBlockIndex uses arith_uint256 nChainWork.

#[derive(Debug, Clone)]
pub struct BlockIndex {
    pub hash: [u8; 32],                // Hash of this block header
    pub prev_hash: Option<[u8; 32]>,   // Hash of the previous block header
    pub height: u32,                   // Height of this block in the chain
    pub header: BlockHeaderData,       // The actual header data

    // Chain work could be more complex for PoW, but for PoS it's often related to sequence/height
    // For simplicity, let's use height as a proxy for chain work in a PoS context for now.
    // A more robust implementation would use a type that can represent accumulated PoW/PoS difficulty.
    pub chain_work_proxy: u64, // Simplified chain work (e.g., sum of difficulties or just height for PoS tip selection)

    // TODO: Add status flags as needed, e.g.:
    // pub status_has_data: bool,       // Whether the full block data is known to be stored
    // pub status_is_valid_tree: bool,  // Header and its ancestors are valid
    // pub status_failed: bool,         // Block or an ancestor is invalid
}

impl BlockIndex {
    pub fn new(header_data: BlockHeaderData, height: u32, prev_hash_opt: Option<[u8; 32]>) -> Self {
        let block_hash = header_data.get_hash(); // Assumes get_hash() is available on BlockHeaderData
        BlockIndex {
            hash: block_hash,
            prev_hash: prev_hash_opt,
            height,
            header: header_data,
            chain_work_proxy: height as u64, // Simplistic chain work for now
        }
    }

    // Helper to get the block time from the header
    pub fn get_block_time(&self) -> u32 {
        self.header.timestamp
    }

    // Placeholder for checking if this block index is the genesis block
    pub fn is_genesis(&self) -> bool {
        self.prev_hash.is_none() && self.height == 0
    }
} 