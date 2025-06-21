use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::net::{SocketAddr, Ipv4Addr, Ipv6Addr};
use crate::p2p::messages::{MasternodeBroadcast, MasternodePing, TxInRaw, DsegMessage, CMD_DSEG, MessageHeader, Encodable};
use crate::storage::BlockStorage;
use crate::spork_manager::SporkManager;
use crate::p2p::connection;
use tokio::net::TcpStream;
use serde::Serialize;
use crate::chainparams;

// The key for the masternodes map, representing the outpoint of the collateral
pub type MasternodeOutPoint = ([u8; 32], u32);

#[derive(Debug, Serialize, Clone)] // For API responses
#[serde(rename_all = "camelCase")]
pub struct ApiMasternodeSummary {
    pub collateral_hash: String,
    pub collateral_index: u32,
    pub address: String, // IP:Port
    pub payee: String,   // TWINS address from pubKeyCollateralAddress
    pub status: String,  // e.g., "ENABLED", "EXPIRED", "PRE_ENABLED", "POSE_BAN"
    pub active_time_seconds: i64, // Approx. active time
    pub last_seen: i64, // Unix timestamp of last ping
    pub protocol_version: i32,
    pub rank: Option<i32>, // Placeholder for payment rank
    // Add other fields as needed by the explorer, e.g., pubkey_masternode
}

#[derive(Debug)]
pub struct MasternodeManager {
    // Store MasternodeBroadcast, keyed by its collateral outpoint (vin.prev_out_hash, vin.prev_out_n)
    masternodes: RwLock<HashMap<MasternodeOutPoint, MasternodeBroadcast>>,
    storage: Arc<dyn BlockStorage>,
    spork_manager: Arc<SporkManager>,
    // TODO: Add fields for tracking requests, sync state, etc.
}

impl MasternodeManager {
    pub fn new(storage: Arc<dyn BlockStorage>, spork_manager: Arc<SporkManager>) -> Self {
        // TODO: Load masternodes from storage on startup
        let loaded_masternodes = match storage.load_masternodes() {
            Ok(mns) => mns,
            Err(e) => {
                log::error!("Failed to load masternodes from DB, starting with empty set: {}", e);
                HashMap::new()
            }
        };

        MasternodeManager {
            masternodes: RwLock::new(loaded_masternodes),
            storage,
            spork_manager,
        }
    }

    // Processes a MasternodeBroadcast message received from a peer
    pub fn process_mnb_message(&self, mnb: MasternodeBroadcast, _source_peer_addr: Option<SocketAddr>) {
        log::info!("Processing MNB for collateral: {}:{}", 
            hex::encode(mnb.vin.prev_out_hash),
            mnb.vin.prev_out_n
        );
        // TODO: Implement CMasternodeBroadcast::CheckAndUpdate equivalent validation:
        // 1. Signature verification (mnb.sig against a hash of the mnb's core fields using mnb.pubkey_collateral_address)
        // 2. Check input age (collateral confirmations)
        // 3. Check protocol version
        // 4. Check last ping validity and timeliness
        // 5. Check if collateral is unspent (requires UTXO lookup or equivalent blockchain state access)
        // If valid:
        //   let key = (mnb.vin.prev_out_hash, mnb.vin.prev_out_n);
        //   let mut masternodes_map = self.masternodes.write().unwrap();
        //   masternodes_map.insert(key, mnb.clone());
        //   if let Err(e) = self.storage.save_masternode(&mnb) {
        //       log::error!("Failed to save masternode {} to DB: {}", hex::encode(key.0), e);
        //   }
        // TODO: Relay valid new/updated MNBs to other peers
    }

    // Processes a MasternodePing message received from a peer
    pub fn process_mnp_message(&self, mnp: MasternodePing, _source_peer_addr: Option<SocketAddr>) {
        log::info!("Processing MNP for collateral: {}:{}", 
            hex::encode(mnp.vin.prev_out_hash),
            mnp.vin.prev_out_n
        );
        // TODO: Implement CMasternodePing::CheckAndUpdate equivalent validation:
        // 1. Signature verification (mnp.vch_sig against mnp.GetHash() using pubkey of the MN in mnp.vin)
        // 2. Check if the masternode is known.
        //    If not known, potentially request its MNB using AskForMN (dseg with specific vin).
        // 3. Update last ping time for the masternode in self.masternodes and storage.
    }

    // Requests the full masternode list from a peer
    // This function will be called by handle_peer_session.
    pub async fn request_masternode_list(&self, stream: &mut TcpStream) -> Result<(), std::io::Error> {
        log::debug!("MasternodeManager: Sending DSEG (full list request) message");
        let dseg_payload_data = DsegMessage {
            vin: TxInRaw { 
                prev_out_hash: [0u8; 32],
                prev_out_n: u32::MAX, 
                script_sig: Vec::new(),
                sequence: u32::MAX, 
            }
        };
        let mut dseg_payload_bytes = Vec::new();
        dseg_payload_data.consensus_encode(&mut std::io::Cursor::new(&mut dseg_payload_bytes))?;
        let checksum = connection::calculate_checksum(&dseg_payload_bytes);
        let header = MessageHeader::new(*CMD_DSEG, dseg_payload_bytes.len() as u32, checksum);
        connection::send_message(stream, header, &dseg_payload_bytes).await;
        // TODO: Manage request state (e.g., track which peers we've asked, timeouts)
        Ok(())
    }

    // New method for API
    pub fn get_masternodes_summary_list(
        &self,
        status_filter: Option<String>,
        // Pagination params to be used later
        _page: u32, 
        _limit: u32,
    ) -> (Vec<ApiMasternodeSummary>, usize) {
        let masternodes_map = self.masternodes.read().unwrap();
        let mut summaries = Vec::new();

        let now = chrono::Utc::now().timestamp();
        // MASTERNODE_PING_SECONDS, MASTERNODE_EXPIRATION_SECONDS would ideally be constants
        const MASTERNODE_EXPIRATION_SECONDS: i64 = 120 * 60;

        for ((_collateral_hash, _collateral_idx), mnb) in masternodes_map.iter() {
            // Simplified status logic for now. C++ CMasternode::GetStatus() is more complex.
            let mut status_str = "UNKNOWN".to_string();
            let mut is_active_for_filter = false;

            // Placeholder: a more detailed status check (like in C++ CMasternode::Status() and IsEnabled()) is needed.
            // This might check protocol version, ping times, sporks, etc.
            // For now, let's assume if it has a recent ping, it's somewhat "active".
            if mnb.last_ping.sig_time > 0 && (now - mnb.last_ping.sig_time) < MASTERNODE_EXPIRATION_SECONDS {
                status_str = "ENABLED".to_string(); // Simplified: active ping implies enabled for now
                is_active_for_filter = true;
            } else if mnb.last_ping.sig_time > 0 {
                status_str = "EXPIRED".to_string();
            }

            if let Some(ref filter) = status_filter {
                let filter_lower = filter.to_lowercase();
                if (filter_lower == "active" || filter_lower == "enabled") && !is_active_for_filter {
                    continue;
                }
            }

            // TODO: Convert CPubKey (Vec<u8>) to TWINS Address string
            // This requires base58check encoding with the correct version byte for TWINS addresses.
            // For now, a placeholder.
            let payee_address = crate::util::pubkey_to_address(&mnb.pubkey_collateral_address, &chainparams::MAINNET_PARAMS);
            
            // Reconstruct IpAddr for proper logging/display (simplified from C++ NetAddr::ToStringIPPort())
            let ip_addr_str = if mnb.addr.ip[0..12] == [0,0,0,0,0,0,0,0,0,0,0xff,0xff] {
                Ipv4Addr::new(mnb.addr.ip[12], mnb.addr.ip[13], mnb.addr.ip[14], mnb.addr.ip[15]).to_string()
            } else {
                Ipv6Addr::from(mnb.addr.ip).to_string()
            };
            let net_addr_str = format!("{}:{}", ip_addr_str, u16::from_be(mnb.addr.port));

            summaries.push(ApiMasternodeSummary {
                collateral_hash: hex::encode(mnb.vin.prev_out_hash),
                collateral_index: mnb.vin.prev_out_n,
                address: net_addr_str, // mnb.addr.ToString() equivalent
                payee: payee_address, // Placeholder
                status: status_str,
                active_time_seconds: if mnb.sig_time > 0 { now.saturating_sub(mnb.sig_time) } else { 0 },
                last_seen: mnb.last_ping.sig_time,
                protocol_version: mnb.protocol_version,
                rank: None, // Placeholder
            });
        }
        let total_items = summaries.len(); // Before actual pagination
        // TODO: Implement actual pagination using page and limit
        (summaries, total_items)
    }

    // New method for fetching a single masternode's full broadcast data
    pub fn get_masternode_detail_by_outpoint(&self, outpoint: &MasternodeOutPoint) -> Option<MasternodeBroadcast> {
        let masternodes_map = self.masternodes.read().unwrap();
        masternodes_map.get(outpoint).cloned() // Clone the MNB if found
    }

    // TODO: Add methods like:
    // get_masternode_count()
    // check_and_remove_expired_masternodes()
} 