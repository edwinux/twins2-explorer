use axum;
use axum::{
    routing::get,
    response::{Json, IntoResponse, Response, Sse},
    Router,
    extract::{State, Path, Query},
    http::{StatusCode, Method, header::{CONTENT_TYPE}},
};
use axum::response::sse::{Event, KeepAlive};
use tower_http::cors::{CorsLayer, Any};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use hex;
use tokio_stream::{wrappers::IntervalStream, StreamExt as _};
use std::time::Duration;

// Import the actual types for the application state
use crate::blockchain::chain_state::ChainState;
use crate::storage::BlockStorage;
use crate::spork_manager::{SporkManager, SporkDetail};
use crate::masternode_manager::{MasternodeManager, ApiMasternodeSummary, MasternodeOutPoint};
use crate::p2p::messages::MasternodeBroadcast;
use crate::governance_manager::{GovernanceManager, ApiBudgetProposalSummary};
use crate::instant_send_manager::InstantSendManager;
use crate::p2p::messages::{BlockHeaderData, TransactionData, TxInRaw, TxOutRaw, Encodable};
use crate::p2p::peer_manager::PeerManager;
use crate::p2p::hybrid_sync::{HybridSyncStatus, SyncMode};

// Shared application state available to all handlers
#[derive(Clone)]
pub struct ApiAppState {
    pub storage: Arc<dyn BlockStorage>,
    pub chain_state: Arc<ChainState>,
    pub spork_manager: Arc<SporkManager>,
    pub masternode_manager: Arc<MasternodeManager>,
    pub governance_manager: Arc<GovernanceManager>,
    pub instant_send_manager: Arc<InstantSendManager>,
    pub peer_manager: Arc<PeerManager>,
    pub parallel_sync_manager: Option<Arc<crate::p2p::parallel_sync_manager::ParallelSyncManager>>,
    pub fast_sync: Option<Arc<crate::p2p::fast_sync::FastSync>>,
    pub orphan_manager: Option<Arc<crate::blockchain::orphan_manager::OrphanManager>>,
    pub hybrid_sync_manager: Option<Arc<crate::p2p::hybrid_sync::HybridSyncManager>>,
}

// --- API Error Handling ---
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    InternalServerError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::InternalServerError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(serde_json::json!({ "error": error_message }))).into_response()
    }
}

// --- Response Structs ---
#[derive(Serialize)]
struct PingResponse {
    message: String,
    status: String,
}

#[derive(Serialize)]
struct StatusResponse {
    status: String,
    version: String, // TODO: Get from Cargo.toml or const
    protocol_version: i32,
    block_height: u32,
    header_height: u32, // Could be same as block_height if fully synced
    estimated_network_height: u32,
    sync_progress: f64, // Percentage (0.0 to 100.0)
    peer_count: usize,
    last_block_time: Option<i64>,
    network: String, // Placeholder, should come from config or params
}

// #[derive(Serialize)]
// struct ConnectionsStatus {
//     total: usize,
//     inbound: usize, // TODO: Need to track this in PeerManager
//     outbound: usize, // TODO: Need to track this in PeerManager
// }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")] // To match common JSON conventions
struct ApiBlockDetail {
    hash: String,
    confirmations: Option<u32>,
    size: usize, // Placeholder, actual block size needs to be calculated or stored
    height: u32,
    version: i32,
    #[serde(rename = "versionHex")]
    version_hex: String, // Optional, can be derived
    merkle_root: String,
    time: i64, // Unix timestamp
    median_time: Option<i64>, // Placeholder
    nonce: u32,
    bits: String, // Hex
    difficulty: Option<f64>, // Placeholder
    chainwork: Option<String>, // Placeholder
    #[serde(rename = "nTx")]
    n_tx: usize,
    previous_block_hash: Option<String>,
    next_block_hash: Option<String>,     // Placeholder, needs chain_state lookup logic
    coinbase_txid: Option<String>,       // If PoW
    coinstake_txid: Option<String>,      // If PoS
    block_signature: Option<String>,     // If PoS
    accumulator_checkpoint: Option<String>,
    tx: Vec<String>, // List of transaction IDs
}

// --- Transaction Detail Response Structs ---
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiScriptSig {
    asm: Option<String>, // Full ASM decompilation can be complex, placeholder
    hex: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiScriptPubKey {
    asm: Option<String>, // Full ASM decompilation can be complex, placeholder
    hex: String,
    req_sigs: Option<usize>,
    #[serde(rename = "type")]
    type_str: String, // e.g., "pubkeyhash", "scripthash", "nonstandard"
    addresses: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiTxIn {
    coinbase: Option<String>,
    txid: Option<String>,
    vout: Option<u32>,
    script_sig: ApiScriptSig,
    sequence: u32,
    // These require looking up the previous output, which is complex without full indexing
    prev_out_value: Option<String>, // decimal string value
    prev_out_address: Option<String>,
    // Could also include the scriptPubKey of the prevout if available
    // prev_out_script_pub_key: Option<ApiScriptPubKey> 
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiTxOut {
    value: String, // decimal string value
    n: u32,
    script_pub_key: ApiScriptPubKey,
    // These require knowing if the output is spent
    spent_txid: Option<String>,
    spent_index: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiTransactionDetail {
    txid: String,
    hash: String, // Often same as txid, but included for explorer compatibility
    version: i32,
    size: usize, // Calculated size of the transaction bytes
    // vsize: usize, // Virtual size, if applicable (usually same as size for non-SegWit)
    locktime: u32,
    vin: Vec<ApiTxIn>,
    vout: Vec<ApiTxOut>,
    blockhash: Option<String>,
    confirmations: Option<u32>,
    time: Option<i64>,        // Block time or first seen time if mempool
    blocktime: Option<i64>,   // Specifically block time
    is_coinbase: bool,
    is_coinstake: bool,
    // Add other relevant fields like InstantSend status later if needed
}

// --- Response Structs (continued) ---
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiBlockSummary {
    hash: String,
    height: u32,
    time: i64, // Unix timestamp
    n_tx: usize,
    // Potentially add miner/staker address if easily available or needed
}

// --- Request Parameter Structs ---
#[derive(Deserialize)]
struct LatestBlocksParams {
    #[serde(default = "default_latest_blocks_count")]
    count: u32,
}

fn default_latest_blocks_count() -> u32 {
    10 // Default to 10 latest blocks
}

// --- Request Parameter Structs (continued) ---
#[derive(Deserialize, Debug)]
pub struct MasternodesListParams {
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "default_page_param")]
    page: u32,
    #[serde(default = "default_limit_param")]
    limit: u32,
}

fn default_page_param() -> u32 { 1 }
fn default_limit_param() -> u32 { 20 }

// --- Peer Information Structs ---
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiPeerInfo {
    address: String,
    user_agent: Option<String>,
    version: Option<i32>,
    start_height: Option<i32>,
    services: Option<u64>,
    connected: bool,
    handshake_completed: bool,
}

// --- Request Parameter Structs (continued) ---
#[derive(Deserialize, Debug)]
pub struct GovernanceProposalsParams {
    #[serde(default = "default_page_param")]
    page: u32,
    #[serde(default = "default_limit_param")]
    limit: u32,
}

// --- Paginated Response Struct (Generic) ---
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    page: u32,
    limit: u32,
    total_items: usize,
    total_pages: u32,
    items: Vec<T>,
}

// --- Internal Helper Functions ---
async fn fetch_and_construct_api_block_detail(
    block_hash_bytes: [u8; 32],
    app_state: &ApiAppState,
) -> Result<ApiBlockDetail, ApiError> {
    let block_index = app_state.storage.get_header(&block_hash_bytes)
        .map_err(|e| ApiError::InternalServerError(format!("DB error getting header: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Block header not found for hash: {}", hex::encode(block_hash_bytes))))?;

    let block_message = app_state.storage.get_block(&block_hash_bytes)
        .map_err(|e| ApiError::InternalServerError(format!("DB error getting block: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Block data not found for hash: {}", hex::encode(block_hash_bytes))))?;

    let current_tip_height = app_state.chain_state.get_tip().map_or(0, |tip| tip.height);
    let confirmations = if block_index.height <= current_tip_height {
        Some(current_tip_height - block_index.height + 1)
    } else {
        None 
    };
    
    // Calculate block size (approximate, as this serializes on the fly)
    // In a real scenario, block size might be stored or calculated more efficiently.
    let mut temp_payload_bytes = Vec::new();
    let block_size = block_message.consensus_encode(&mut std::io::Cursor::new(&mut temp_payload_bytes)).unwrap_or(0);

    Ok(ApiBlockDetail {
        hash: hex::encode(block_hash_bytes),
        confirmations,
        size: block_size, 
        height: block_index.height,
        version: block_message.header.version,
        version_hex: format!("{:08x}", block_message.header.version),
        merkle_root: hex::encode(block_message.header.merkle_root),
        time: block_message.header.timestamp as i64,
        median_time: None, // TODO
        nonce: block_message.header.nonce,
        bits: format!("{:08x}", block_message.header.bits),
        difficulty: None, // TODO: Calculate difficulty
        chainwork: None, // TODO: Get actual chainwork
        n_tx: block_message.transactions.len(),
        previous_block_hash: block_index.prev_hash.map(hex::encode),
        next_block_hash: None, // TODO: Implement logic to find next block hash from ChainState or Storage
        coinbase_txid: if !block_message.transactions.is_empty() && block_message.transactions[0].vin.get(0).map_or(false, |vin| vin.prev_out_hash == [0u8;32]) {
            Some(hex::encode(block_message.transactions[0].get_txid()))
        } else { None },
        coinstake_txid: if block_message.transactions.len() > 1 && block_message.transactions[1].is_coinstake() {
            Some(hex::encode(block_message.transactions[1].get_txid()))
        } else { None },
        block_signature: block_message.block_sig.as_ref().map(hex::encode),
        accumulator_checkpoint: block_message.header.accumulator_checkpoint.map(hex::encode),
        tx: block_message.transactions.iter().map(|tx| hex::encode(tx.get_txid())).collect(),
    })
}

// --- Handlers ---
async fn ping_handler() -> Json<PingResponse> {
    log::info!("API /ping endpoint called");
    Json(PingResponse {
        message: "TWINS Node API is running".to_string(),
        status: "ok".to_string(),
    })
}

async fn status_handler(State(app_state): State<ApiAppState>) -> Json<StatusResponse> {
    log::info!("API /status endpoint called");
    let chain_tip = app_state.chain_state.get_tip();
    let block_height = chain_tip.as_ref().map_or(0, |tip| tip.height);
    let header_height = block_height; 
    let last_block_time = chain_tip.map(|tip| tip.header.timestamp as i64);

    let peer_count = app_state.peer_manager.get_peer_count();

    // Determine actual sync status
    let status = determine_sync_status(block_height, last_block_time, peer_count);
    
    // Get actual network height from peers instead of estimating
    let estimated_network_height = get_network_height_from_peers(&app_state, block_height).await;
    let sync_progress = if estimated_network_height > 0 {
        ((block_height as f64 / estimated_network_height as f64) * 100.0).min(100.0)
    } else {
        0.0
    };

    Json(StatusResponse {
        status,
        version: format!("twins-node-rust/{}", env!("CARGO_PKG_VERSION")), // Get from Cargo.toml
        protocol_version: crate::p2p::messages::PROTOCOL_VERSION, // Assuming this is pub
        block_height,
        header_height,
        estimated_network_height,
        sync_progress,
        peer_count,
        last_block_time,
        network: "mainnet".to_string(), // Placeholder
    })
}

#[derive(Serialize)]
struct TorrentSyncResponse {
    is_active: bool,
    total_segments: u32,
    completed_segments: u32,
    downloading_segments: u32,
    waiting_segments: u32,
    failed_segments: u32,
    blocks_per_second: f64,
    visual_progress: String,
    active_downloads: Vec<SegmentInfo>,
    elapsed_seconds: f64,
    total_downloaded_blocks: u32,
    total_synced_blocks: u32,
    sync_range: (u32, u32),
}

#[derive(Serialize)]
struct SegmentInfo {
    start_height: u32,
    end_height: u32,
    completion_percentage: f64,
    peer_addr: Option<String>,
}

async fn torrent_sync_handler(State(app_state): State<ApiAppState>) -> Json<TorrentSyncResponse> {
    log::info!("API /torrent-sync endpoint called");
    
    if let Some(ref psm) = app_state.parallel_sync_manager {
        let status = psm.get_detailed_sync_status().await;
        
        let active_downloads: Vec<SegmentInfo> = status.active_downloads.iter()
            .map(|seg| SegmentInfo {
                start_height: seg.start_height,
                end_height: seg.end_height,
                completion_percentage: seg.completion_percentage,
                peer_addr: seg.peer_addr.clone(),
            })
            .collect();

        Json(TorrentSyncResponse {
            is_active: status.is_active,
            total_segments: status.total_segments,
            completed_segments: status.completed_segments,
            downloading_segments: status.downloading_segments,
            waiting_segments: status.waiting_segments,
            failed_segments: status.failed_segments,
            blocks_per_second: status.blocks_per_second,
            visual_progress: status.visual_progress,
            active_downloads,
            elapsed_seconds: status.elapsed_seconds,
            total_downloaded_blocks: status.total_downloaded_blocks,
            total_synced_blocks: status.total_synced_blocks,
            sync_range: status.sync_range,
        })
    } else {
        // No parallel sync manager available
        Json(TorrentSyncResponse {
            is_active: false,
            total_segments: 0,
            completed_segments: 0,
            downloading_segments: 0,
            waiting_segments: 0,
            failed_segments: 0,
            blocks_per_second: 0.0,
            visual_progress: "·".repeat(60),
            active_downloads: vec![],
            elapsed_seconds: 0.0,
            total_downloaded_blocks: 0,
            total_synced_blocks: 0,
            sync_range: (0, 0),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HybridSyncResponse {
    sync_mode: String,
    total_blocks_received: u64,
    total_blocks_processed: u64,
    blocks_in_queue: u64,
    orphan_blocks: u64,
    blocks_per_second: f64,
    elapsed_seconds: f64,
    rust_peer_count: u32,
    cpp_peer_count: u32,
    fast_sync_active: bool,
    mode_description: String,
}

async fn hybrid_sync_handler(State(app_state): State<ApiAppState>) -> Json<HybridSyncResponse> {
    log::info!("API /hybrid-sync endpoint called");
    
    if let Some(ref hsm) = app_state.hybrid_sync_manager {
        let status = hsm.get_sync_status().await;
        
        let mode_str = match &status.sync_mode {
            SyncMode::Standard => "standard",
            SyncMode::Fast => "fast",
            SyncMode::Hybrid => "hybrid",
        };
        
        let mode_description = match &status.sync_mode {
            SyncMode::Standard => format!("Standard sync from {} C++ nodes", status.cpp_peer_count),
            SyncMode::Fast => format!("Fast sync from {} Rust nodes", status.rust_peer_count),
            SyncMode::Hybrid => format!("Hybrid sync from {} Rust + {} C++ nodes", status.rust_peer_count, status.cpp_peer_count),
        };
        
        Json(HybridSyncResponse {
            sync_mode: mode_str.to_string(),
            total_blocks_received: status.total_blocks_received,
            total_blocks_processed: status.total_blocks_processed,
            blocks_in_queue: status.blocks_in_queue,
            orphan_blocks: status.orphan_blocks,
            blocks_per_second: status.blocks_per_second,
            elapsed_seconds: status.elapsed_seconds,
            rust_peer_count: status.rust_peer_count,
            cpp_peer_count: status.cpp_peer_count,
            fast_sync_active: status.fast_sync_active,
            mode_description,
        })
    } else {
        // No hybrid sync manager available
        Json(HybridSyncResponse {
            sync_mode: "disabled".to_string(),
            total_blocks_received: 0,
            total_blocks_processed: 0,
            blocks_in_queue: 0,
            orphan_blocks: 0,
            blocks_per_second: 0.0,
            elapsed_seconds: 0.0,
            rust_peer_count: 0,
            cpp_peer_count: 0,
            fast_sync_active: false,
            mode_description: "Hybrid sync manager not available".to_string(),
        })
    }
}

fn determine_sync_status(block_height: u32, last_block_time: Option<i64>, peer_count: usize) -> String {
    let now = chrono::Utc::now().timestamp();
    
    // If no peers, we're definitely not synced
    if peer_count == 0 {
        return "disconnected".to_string();
    }
    
    // Check if last block is too old (more than 30 minutes indicates we're behind)
    if let Some(last_time) = last_block_time {
        let block_age_minutes = (now - last_time) / 60;
        
        // If last block is more than 30 minutes old, we're likely not synced
        if block_age_minutes > 30 {
            return "syncing".to_string();
        }
        
        // If last block is very recent (less than 10 minutes), we're likely synced
        if block_age_minutes < 10 {
            return "synced".to_string();
        }
    }
    
    // For TWINS network, blocks should be generated regularly
    // If we have recent blocks and peers, we're likely synced
    if peer_count >= 3 {
        return "synced".to_string();
    }
    
    // Default to syncing if we're uncertain
    "syncing".to_string()
}

// Get the actual network height from connected peers
async fn get_network_height_from_peers(app_state: &ApiAppState, current_height: u32) -> u32 {
    // Try to get the best height from peers first
    if let Some(best_peer_height) = app_state.peer_manager.get_best_peer_height().await {
        // Use the highest peer height, but add a small buffer for recent blocks
        // that peers might not have reported yet
        return std::cmp::max(best_peer_height, current_height);
    }
    
    // Fallback to known mainnet height if no peer data available
    const KNOWN_MAINNET_HEIGHT: u32 = 1_613_126; // As of June 2025
    
    // If we're syncing historical blocks, use the known mainnet height
    if current_height < KNOWN_MAINNET_HEIGHT {
        return KNOWN_MAINNET_HEIGHT;
    }
    
    // If we're caught up or ahead, assume we're at the current height
    current_height
}

async fn get_block_by_hash_handler(
    Path(block_hash_hex): Path<String>,
    State(app_state): State<ApiAppState>,
) -> Result<Json<ApiBlockDetail>, ApiError> {
    log::info!("API /block/hash/{} called", block_hash_hex);
    let mut hash_bytes = [0u8; 32];
    if hex::decode_to_slice(&block_hash_hex, &mut hash_bytes).is_err() {
        return Err(ApiError::BadRequest("Invalid block hash format".to_string()));
    }
    let api_block_detail = fetch_and_construct_api_block_detail(hash_bytes, &app_state).await?;
    Ok(Json(api_block_detail))
}

async fn get_block_by_height_handler(
    Path(height): Path<u32>,
    State(app_state): State<ApiAppState>,
) -> Result<Json<ApiBlockDetail>, ApiError> {
    log::info!("API /block/height/{} called", height);

    let block_hash_bytes = app_state.storage.get_block_hash_by_height(height)
        .map_err(|e| ApiError::InternalServerError(format!("DB error getting block hash by height: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Block not found at height: {}", height)))?;

    let api_block_detail = fetch_and_construct_api_block_detail(block_hash_bytes, &app_state).await?;
    Ok(Json(api_block_detail))
}

async fn get_tx_by_id_handler(
    Path(txid_hex): Path<String>,
    State(app_state): State<ApiAppState>,
) -> Result<Json<ApiTransactionDetail>, ApiError> {
    log::info!("API /tx/{} called", txid_hex);

    let mut txid_bytes = [0u8; 32];
    if hex::decode_to_slice(&txid_hex, &mut txid_bytes).is_err() {
        return Err(ApiError::BadRequest("Invalid transaction ID format".to_string()));
    }

    match app_state.storage.get_transaction_details(&txid_bytes) {
        Ok(Some((tx_data, block_hash_opt, block_height_opt))) => {
            let mut confirmations = None;
            let mut block_time_opt = None;

            if let Some(bh) = block_height_opt {
                let current_tip_height = app_state.chain_state.get_tip().map_or(0, |tip| tip.height);
                if bh <= current_tip_height {
                    confirmations = Some(current_tip_height - bh + 1);
                }
                // Try to get block time from header if block_hash is available
                if let Some(b_hash) = block_hash_opt {
                    if let Ok(Some(header_index)) = app_state.storage.get_header(&b_hash) {
                        block_time_opt = Some(header_index.header.timestamp as i64);
                    }
                }
            }
            
            let mut vins_api: Vec<ApiTxIn> = Vec::new();
            for vin_raw in &tx_data.vin {
                vins_api.push(ApiTxIn {
                    coinbase: if vin_raw.prev_out_hash == [0u8; 32] { Some(hex::encode(&vin_raw.script_sig)) } else { None },
                    txid: if vin_raw.prev_out_hash != [0u8; 32] { Some(hex::encode(vin_raw.prev_out_hash)) } else { None },
                    vout: if vin_raw.prev_out_hash != [0u8; 32] { Some(vin_raw.prev_out_n) } else { None },
                    script_sig: ApiScriptSig {
                        asm: None, // Placeholder
                        hex: hex::encode(&vin_raw.script_sig),
                    },
                    sequence: vin_raw.sequence,
                    prev_out_value: None, // Requires index
                    prev_out_address: None, // Requires index
                });
            }

            let mut vouts_api: Vec<ApiTxOut> = Vec::new();
            for (i, vout_raw) in tx_data.vout.iter().enumerate() {
                vouts_api.push(ApiTxOut {
                    value: (vout_raw.value as f64 / 100_000_000.0).to_string(), // Assuming 8 decimal places
                    n: i as u32,
                    script_pub_key: ApiScriptPubKey {
                        asm: None, // Placeholder
                        hex: hex::encode(&vout_raw.script_pubkey),
                        req_sigs: None, // Placeholder
                        type_str: "nonstandard".to_string(), // Placeholder
                        addresses: None, // Placeholder
                    },
                    spent_txid: None, // Requires index
                    spent_index: None, // Requires index
                });
            }

            let mut tx_bytes_for_size = Vec::new();
            let tx_size = tx_data.consensus_encode(&mut std::io::Cursor::new(&mut tx_bytes_for_size)).unwrap_or(0);

            let response = ApiTransactionDetail {
                txid: txid_hex.clone(),
                hash: txid_hex, // Often the same as txid
                version: tx_data.version,
                size: tx_size,
                locktime: tx_data.lock_time,
                vin: vins_api,
                vout: vouts_api,
                blockhash: block_hash_opt.map(hex::encode),
                confirmations,
                time: block_time_opt, // Use block time if available
                blocktime: block_time_opt,
                is_coinbase: !tx_data.vin.is_empty() && tx_data.vin[0].prev_out_hash == [0u8; 32],
                is_coinstake: tx_data.is_coinstake(),
            };
            Ok(Json(response))
        }
        Ok(None) => Err(ApiError::NotFound(format!("Transaction not found: {}", txid_hex))),
        Err(e) => Err(ApiError::InternalServerError(format!("DB error getting transaction: {}", e))),
    }
}

async fn get_latest_blocks_handler(
    Query(params): Query<LatestBlocksParams>,
    State(app_state): State<ApiAppState>,
) -> Result<Json<Vec<ApiBlockSummary>>, ApiError> {
    let count = std::cmp::min(params.count, 100); // Cap count at 100 for safety
    log::info!("API /blocks/latest called with count: {}", count);

    let current_tip_height = app_state.chain_state.get_tip().map_or(0, |tip| tip.height);
    if current_tip_height == 0 && count > 0 { // No blocks or only genesis
        if app_state.storage.get_header(&app_state.chain_state.genesis_hash).unwrap_or(None).is_some() && count >=1 {
            // Only genesis exists, return it if count >= 1
        } else {
            return Ok(Json(Vec::new())); // No blocks to return yet other than possibly genesis
        }
    }

    let block_hashes = app_state.storage.get_block_hashes_by_height_range(current_tip_height, count)
        .map_err(|e| ApiError::InternalServerError(format!("DB error getting latest block hashes: {}", e)))?;

    let mut block_summaries = Vec::new();

    for block_hash_bytes in block_hashes {
        // We need BlockIndex for height & time, and BlockMessage for nTx.
        // fetch_and_construct_api_block_detail gets full details, which is overkill here.
        // Let's fetch header and block message separately for summary.
        match app_state.storage.get_header(&block_hash_bytes) {
            Ok(Some(block_index)) => {
                match app_state.storage.get_block(&block_hash_bytes) {
                    Ok(Some(block_message)) => {
                        block_summaries.push(ApiBlockSummary {
                            hash: hex::encode(block_hash_bytes),
                            height: block_index.height,
                            time: block_message.header.timestamp as i64,
                            n_tx: block_message.transactions.len(),
                        });
                    }
                    Ok(None) => {
                        log::warn!("Block data not found for hash {} while getting latest blocks summary.", hex::encode(block_hash_bytes));
                        // Skip this block or return partial data? For now, skip.
                    }
                    Err(e) => {
                        log::error!("DB error getting block data for {}: {}. Skipping.", hex::encode(block_hash_bytes), e);
                        // Skip this block
                    }
                }
            }
            Ok(None) => {
                 log::warn!("Block header not found for hash {} while getting latest blocks summary.", hex::encode(block_hash_bytes));
                // Skip this block
            }
            Err(e) => {
                log::error!("DB error getting header for {}: {}. Skipping.", hex::encode(block_hash_bytes), e);
                // Skip this block
            }
        }
    }
    Ok(Json(block_summaries))
}

async fn get_sporks_handler(
    State(app_state): State<ApiAppState>,
) -> Result<Json<Vec<SporkDetail>>, ApiError> {
    log::info!("API /sporks endpoint called");
    let spork_details = app_state.spork_manager.get_all_spork_details();
    Ok(Json(spork_details))
}

async fn get_masternodes_list_handler(
    Query(params): Query<MasternodesListParams>,
    State(app_state): State<ApiAppState>,
) -> Result<Json<PaginatedResponse<ApiMasternodeSummary>>, ApiError> {
    log::info!("API /masternodes/list called with params: {:?}", params);

    let page = if params.page == 0 { 1 } else { params.page }; // Ensure page is at least 1
    let limit = std::cmp::min(params.limit, 100); // Cap limit

    // get_masternodes_summary_list currently returns all matching items and total.
    // True pagination would be applied here based on page & limit if the manager method supported it.
    // For now, we'll fetch all and simulate pagination for the response structure.
    let (all_matching_summaries, total_items) = 
        app_state.masternode_manager.get_masternodes_summary_list(params.status, page, limit);

    // Simulate pagination for the response (actual DB/manager pagination is TODO)
    let total_pages = (total_items as f64 / limit as f64).ceil() as u32;
    let offset = ((page - 1) * limit) as usize;
    let paginated_items: Vec<ApiMasternodeSummary> = all_matching_summaries
        .into_iter()
        .skip(offset)
        .take(limit as usize)
        .collect();

    Ok(Json(PaginatedResponse {
        page,
        limit,
        total_items,
        total_pages,
        items: paginated_items,
    }))
}

fn parse_outpoint_str(outpoint_str: &str) -> Result<MasternodeOutPoint, ApiError> {
    let parts: Vec<&str> = outpoint_str.split(':').collect();
    if parts.len() != 2 {
        return Err(ApiError::BadRequest("Invalid outpoint format. Expected txid:index".to_string()));
    }
    let mut txid_bytes = [0u8; 32];
    if hex::decode_to_slice(parts[0], &mut txid_bytes).is_err() {
        return Err(ApiError::BadRequest("Invalid txid hex in outpoint".to_string()));
    }
    let index = parts[1].parse::<u32>()
        .map_err(|_| ApiError::BadRequest("Invalid index in outpoint".to_string()))?;
    Ok((txid_bytes, index))
}

async fn get_masternode_detail_handler(
    Path(outpoint_str): Path<String>,
    State(app_state): State<ApiAppState>,
) -> Result<Json<MasternodeBroadcast>, ApiError> { // Returns the full MasternodeBroadcast
    log::info!("API /masternode/{} called", outpoint_str);

    let outpoint = parse_outpoint_str(&outpoint_str)?;

    match app_state.masternode_manager.get_masternode_detail_by_outpoint(&outpoint) {
        Some(mnb) => Ok(Json(mnb)),
        None => Err(ApiError::NotFound(format!("Masternode not found for outpoint: {}", outpoint_str))),
    }
}

async fn get_governance_proposals_handler(
    Query(params): Query<GovernanceProposalsParams>,
    State(app_state): State<ApiAppState>,
) -> Result<Json<PaginatedResponse<ApiBudgetProposalSummary>>, ApiError> {
    log::info!("API /governance/proposals called with params: {:?}", params);

    let page = if params.page == 0 { 1 } else { params.page };
    let limit = std::cmp::min(params.limit, 100); // Cap limit

    let (all_matching_summaries, total_items) = 
        app_state.governance_manager.get_proposals_summary_list(page, limit);

    // Simulate pagination for the response (actual DB/manager pagination is TODO in manager)
    let total_pages = (total_items as f64 / limit as f64).ceil() as u32;
    let offset = ((page - 1) * limit) as usize;
    let paginated_items: Vec<ApiBudgetProposalSummary> = all_matching_summaries
        .into_iter()
        .skip(offset)
        .take(limit as usize)
        .collect();

    Ok(Json(PaginatedResponse {
        page,
        limit,
        total_items,
        total_pages,
        items: paginated_items,
    }))
}

async fn get_peers_handler(
    State(app_state): State<ApiAppState>,
) -> Result<Json<Vec<ApiPeerInfo>>, ApiError> {
    log::info!("API /peers endpoint called");
    
    // For now, we'll return basic peer information
    // In a full implementation, we'd need to add methods to PeerManager to iterate over peers
    let peer_count = app_state.peer_manager.get_peer_count();
    
    // Since we don't have direct access to peer details yet, return basic info
    let peers_info = vec![]; // Placeholder - would need PeerManager enhancement
    
    Ok(Json(peers_info))
}

async fn status_stream_handler(
    State(app_state): State<ApiAppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>> {
    log::info!("API /status/stream endpoint called - starting real-time stream");
    
    let stream = IntervalStream::new(tokio::time::interval(Duration::from_secs(2)))
        .map(move |_| {
            let app_state = app_state.clone();
            async move {
                let chain_tip = app_state.chain_state.get_tip();
                let block_height = chain_tip.as_ref().map_or(0, |tip| tip.height);
                let header_height = block_height; 
                let last_block_time = chain_tip.map(|tip| tip.header.timestamp as i64);
                let peer_count = app_state.peer_manager.get_peer_count();
                
                let status = determine_sync_status(block_height, last_block_time, peer_count);
                let estimated_network_height = get_network_height_from_peers(&app_state, block_height).await;
                let sync_progress = if estimated_network_height > 0 {
                    ((block_height as f64 / estimated_network_height as f64) * 100.0).min(100.0)
                } else {
                    0.0
                };

                let status_response = StatusResponse {
                    status,
                    version: format!("twins-node-rust/{}", env!("CARGO_PKG_VERSION")),
                    protocol_version: crate::p2p::messages::PROTOCOL_VERSION,
                    block_height,
                    header_height,
                    estimated_network_height,
                    sync_progress,
                    peer_count,
                    last_block_time,
                    network: "mainnet".to_string(),
                };

                match serde_json::to_string(&status_response) {
                    Ok(json_data) => Ok(Event::default().data(json_data)),
                    Err(e) => {
                        log::error!("Failed to serialize status for SSE: {}", e);
                        Ok(Event::default().data("{\"error\":\"serialization_failed\"}"))
                    }
                }
            }
        })
        .then(|future| future)
        .map(|result| result);

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// Function to create the Axum router with all API routes
pub fn create_router(app_state: ApiAppState) -> Router {
    // Configure CORS to allow the frontend to access the API
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([CONTENT_TYPE]);

    Router::new()
        .route("/api/v1/ping", get(ping_handler))
        .route("/api/v1/status", get(status_handler))
        .route("/api/v1/block/hash/:block_hash", get(get_block_by_hash_handler))
        .route("/api/v1/block/height/:height", get(get_block_by_height_handler))
        .route("/api/v1/tx/:txid", get(get_tx_by_id_handler))
        .route("/api/v1/blocks/latest", get(get_latest_blocks_handler))
        .route("/api/v1/sporks", get(get_sporks_handler))
        .route("/api/v1/masternodes/list", get(get_masternodes_list_handler))
        .route("/api/v1/masternode/:outpoint", get(get_masternode_detail_handler))
        .route("/api/v1/governance/proposals", get(get_governance_proposals_handler))
        .route("/api/v1/peers", get(get_peers_handler))
        .route("/api/v1/status/stream", get(status_stream_handler))
        .route("/api/v1/torrent-sync", get(torrent_sync_handler))
        .route("/api/v1/hybrid-sync", get(hybrid_sync_handler))
        .layer(cors)
        .with_state(app_state)
}

// The main function to run the API service will be called from main.rs
// It will take the app_state as an argument.
// pub async fn run_api_service(app_state: ApiAppState) -> Result<(), Box<dyn std::error::Error>> {
//     let app = create_router(app_state);
//     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
//     log::info!("API server listening on {}", listener.local_addr()?);
//     axum::serve(listener, app).await?;
//     Ok(())
// } 