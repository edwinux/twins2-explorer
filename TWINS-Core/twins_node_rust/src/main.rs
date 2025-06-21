use env_logger;
use log;
use std::sync::Arc;

mod p2p;
mod blockchain;
mod storage;
mod spork_manager;
mod masternode_manager;
mod governance_manager;
mod instant_send_manager;
mod api_service;
mod util;
mod chainparams;

// Import the connection function and PeerManager
use p2p::connection::try_connect_to_seeds;
use p2p::messages::NODE_NETWORK; // Assuming our node provides basic network services
use p2p::peer_manager::PeerManager;
use p2p::parallel_sync_manager::ParallelSyncManager;
use p2p::hybrid_sync::HybridSyncManager;
use blockchain::chain_state::ChainState;
use storage::{SqliteBlockStorage, BlockStorage};
use spork_manager::SporkManager;
use masternode_manager::MasternodeManager;
use governance_manager::GovernanceManager;
use instant_send_manager::InstantSendManager;
use api_service::ApiAppState;

const DB_PATH: &str = "twins_node_data.sqlite";
const API_LISTEN_ADDR: &str = "0.0.0.0:3001";

// Define the actual application state for the API
#[derive(Clone)]
struct AppState {
    storage: Arc<dyn BlockStorage>,
    chain_state: Arc<ChainState>,
    spork_manager: Arc<SporkManager>,
    masternode_manager: Arc<MasternodeManager>,
    governance_manager: Arc<GovernanceManager>,
    instant_send_manager: Arc<InstantSendManager>,
}

// Implement the marker trait defined in api_service.rs if it were pub
// For now, we assume api_service::ApiAppState will be adapted or this AppState will be passed directly.
// To make this work with the current api_service.rs, ApiAppState in api_service.rs needs to be:
// pub struct ApiAppState {
//   pub storage: Arc<dyn crate::storage::BlockStorage>,
//   pub chain_state: Arc<crate::blockchain::chain_state::ChainState>,
//   ...
// }
// Or, we pass this AppState struct and handlers in api_service.rs use axum::extract::State<AppState>

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Initialize logger
    env_logger::init();

    log::info!("TWINS Rust Node starting up...");

    // Create Storage instance
    let storage_instance: Arc<dyn BlockStorage> = match SqliteBlockStorage::new(DB_PATH) {
        Ok(db) => Arc::new(db),
        Err(e) => {
            log::error!("Failed to initialize node database: {}", e);
            // Using std::io::Error::new for simplicity. Consider a custom error type.
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("DB init error: {}", e)));
        }
    };

    let spork_manager_arc = Arc::new(SporkManager::new(Arc::clone(&storage_instance)));
    let masternode_manager_arc = Arc::new(MasternodeManager::new(Arc::clone(&storage_instance), Arc::clone(&spork_manager_arc)));
    let governance_manager_arc = Arc::new(GovernanceManager::new(Arc::clone(&storage_instance), Arc::clone(&spork_manager_arc)));
    let instant_send_manager_arc = Arc::new(InstantSendManager::new(Arc::clone(&storage_instance), Arc::clone(&spork_manager_arc), Arc::clone(&masternode_manager_arc)));
    let chain_state_arc = Arc::new(ChainState::new(Arc::clone(&storage_instance)));
    let peer_manager_arc = Arc::new(PeerManager::new());
    
    // Initialize orphan manager for handling out-of-order blocks
    let orphan_manager_arc = Arc::new(crate::blockchain::orphan_manager::OrphanManager::new(
        Arc::clone(&chain_state_arc),
        Arc::clone(&storage_instance),
    ));
    
    // Initialize fast sync for high-performance parallel downloading
    let fast_sync_arc = Arc::new(crate::p2p::fast_sync::FastSync::new(
        Arc::clone(&chain_state_arc),
        Arc::clone(&storage_instance),
        Arc::clone(&orphan_manager_arc),
    ));
    
    // Initialize parallel sync manager for torrent-like multi-threaded sync
    let parallel_sync_manager_arc = Arc::new(ParallelSyncManager::new(
        Arc::clone(&chain_state_arc),
        Arc::clone(&storage_instance),
    ));
    
    // Initialize hybrid sync manager for intelligent sync mode switching
    let hybrid_sync_manager_arc = Arc::new(HybridSyncManager::new(
        Arc::clone(&chain_state_arc),
        Arc::clone(&storage_instance),
        Arc::clone(&orphan_manager_arc),
    ));

    // TODO: Load configuration (e.g., our node's services, start height if known from DB)
    let our_start_height = chain_state_arc.get_tip().map_or(0, |tip| tip.height);
    let our_services = NODE_NETWORK; // Placeholder

    // --- Setup Application State for API ---
    // This state will be passed to Axum handlers.
    // For this to compile with the current api_service.rs, you'd need to make ApiAppState in api_service.rs public
    // and ensure its fields match what you need, or adapt how create_router takes state.
    // Let's assume api_service.rs is adapted to use this AppState or a compatible one.
    let app_state = ApiAppState {
        storage: Arc::clone(&storage_instance),
        chain_state: Arc::clone(&chain_state_arc),
        spork_manager: Arc::clone(&spork_manager_arc),
        masternode_manager: Arc::clone(&masternode_manager_arc),
        governance_manager: Arc::clone(&governance_manager_arc),
        instant_send_manager: Arc::clone(&instant_send_manager_arc),
        peer_manager: Arc::clone(&peer_manager_arc),
        parallel_sync_manager: Some(Arc::clone(&parallel_sync_manager_arc)),
        fast_sync: Some(Arc::clone(&fast_sync_arc)),
        orphan_manager: Some(Arc::clone(&orphan_manager_arc)),
        hybrid_sync_manager: Some(Arc::clone(&hybrid_sync_manager_arc)),
    };

    // --- Start P2P Networking Task ---
    let p2p_task = tokio::spawn({
        let pm_clone = Arc::clone(&peer_manager_arc);
        let cs_clone = Arc::clone(&chain_state_arc);
        let storage_clone_for_p2p = Arc::clone(&storage_instance);
        let spork_manager_clone_for_p2p = Arc::clone(&spork_manager_arc);
        let mn_manager_clone_for_p2p = Arc::clone(&masternode_manager_arc);
        let gov_manager_clone_for_p2p = Arc::clone(&governance_manager_arc);
        let ix_manager_clone_for_p2p = Arc::clone(&instant_send_manager_arc);
        let psm_clone_for_p2p = Arc::clone(&parallel_sync_manager_arc);
        let hsm_clone_for_p2p = Arc::clone(&hybrid_sync_manager_arc);
        async move {
            try_connect_to_seeds(
                pm_clone, cs_clone, storage_clone_for_p2p, 
                spork_manager_clone_for_p2p, mn_manager_clone_for_p2p,
                gov_manager_clone_for_p2p, ix_manager_clone_for_p2p,
                Some(psm_clone_for_p2p),
                Some(hsm_clone_for_p2p),
                our_start_height as i32, our_services
            ).await;
        }
    });
    log::info!("P2P connection attempts dispatched.");

    // --- Start Hybrid Sync Manager ---
    let _hybrid_sync_task = tokio::spawn({
        let hsm_clone = Arc::clone(&hybrid_sync_manager_arc);
        async move {
            hsm_clone.start().await;
        }
    });
    log::info!("🔀 Hybrid sync manager started - will intelligently switch between standard and fast sync modes.");
    
    // --- Start Fast Sync ---
    let _fast_sync_task = tokio::spawn({
        let fs_clone = Arc::clone(&fast_sync_arc);
        let cs_clone = Arc::clone(&chain_state_arc);
        let pm_clone = Arc::clone(&peer_manager_arc);
        async move {
            // Wait for peers to connect
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            
            let current_height = cs_clone.get_tip().map_or(0, |tip| tip.height);
            
            // Get the best height from connected peers
            if let Some(target_height) = pm_clone.get_best_peer_height().await {
                if target_height > current_height + 100 { // Only start if significantly behind
                    log::info!("🚀 Starting FastSync from {} to {} ({} blocks)", 
                              current_height, target_height, target_height - current_height);
                    fs_clone.start(target_height).await;
                    
                    // Feed initial blocks to download based on what we know
                    // In a real implementation, this would come from headers or inventory
                    // For now, just request a range of blocks
                    let mut blocks_to_request = Vec::new();
                    for _height in (current_height + 1)..=std::cmp::min(current_height + 10000, target_height) {
                        // This is a placeholder - in reality we'd need the actual block hashes
                        // For testing, we'll let the INV messages provide the hashes
                        blocks_to_request.push([0u8; 32]); // Placeholder
                    }
                    
                    if !blocks_to_request.is_empty() {
                        log::info!("Queuing {} blocks for download", blocks_to_request.len());
                        // fs_clone.add_blocks_to_download(blocks_to_request).await;
                    }
                } else {
                    log::info!("Node is nearly synced (height: {}), using regular sync", current_height);
                }
            } else {
                log::warn!("No peer height information available, using regular sync");
            }
        }
    });
    log::info!("Fast sync manager initialized.");

    // --- Start API Service Task ---
    let api_router = api_service::create_router(app_state); // Use the function from api_service.rs
    let api_listener = match tokio::net::TcpListener::bind(API_LISTEN_ADDR).await {
        Ok(listener) => listener,
        Err(e) => {
            log::error!("Failed to bind API listener to {}: {}", API_LISTEN_ADDR, e);
            // Consider if this should be a fatal error for the whole application
            p2p_task.abort(); // Abort P2P task if API can't start
            return Err(e);
        }
    };
    log::info!("API server listening on {}", API_LISTEN_ADDR);
    let api_task = tokio::spawn(async move {
        match axum::serve(api_listener, api_router.into_make_service()).await {
            Ok(_) => log::info!("API service stopped normally."),
            Err(e) => log::error!("API service encountered an error: {}", e),
        }
    });

    // --- Wait for Ctrl-C (keep both tasks running) ---
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            log::info!("Ctrl-C received, TWINS Rust Node shutting down...");
        }
        res_api = api_task => {
            match res_api {
                Ok(_) => log::info!("API service completed gracefully."),
                Err(e) => log::error!("API service task panicked or was cancelled: {}", e),
            }
        }
    }
    
    // The P2P task runs in background and doesn't cause shutdown
    // Check if P2P task is still running and log its status
    if !p2p_task.is_finished() {
        log::info!("P2P task still running, aborting...");
        p2p_task.abort();
    } else {
        match p2p_task.await {
            Ok(_) => log::info!("P2P task had already completed."),
            Err(e) => log::error!("P2P task failed: {}", e),
        }
    }

    // TODO: Add graceful shutdown logic for peers and other services here.
    log::info!("TWINS Rust Node shut down complete.");
    Ok(())
} 