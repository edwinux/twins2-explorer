use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::p2p::messages::SporkMessage;
use crate::storage::BlockStorage;
use std::net::SocketAddr; // For peer_addr in process_spork_message
use hex;
use secp256k1::{Secp256k1, Message as SecpMessage, PublicKey, ecdsa::Signature, Error as SecpError};
use sha2::{Sha256, Digest};
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::Write; // For WriteBytesExt
use chrono; // Ensure chrono is imported
use serde::Serialize; // For SporkDetail

// Mainnet Spork Keys and Timestamps from TWINS-Core/src/chainparams.cpp
const MAINNET_SPORK_KEY_NEW: &str = "0496c1186ed9170fe353a6287c6f2b1ec768dcfc0fe71943067a0c21349dcf22af77a37f3202d540e85092ad4179f9b44806269450cd0982071ea7a9375ac7d949";
const MAINNET_SPORK_KEY_OLD: &str = "04d8ef5cc6ef836335a868be72cf1fa97bb2628a36febc54c004809259b64f2cc8b0dacfd72ca69b3a692c719672ca4f2cbbd7cdd140ad3e1544479ea378a21cc2";
const MAINNET_ENFORCE_NEW_SPORK_KEY: i64 = 1547424000;
const MAINNET_REJECT_OLD_SPORK_KEY: i64 = 1547510400;

// Default Spork Values from TWINS-Core/src/spork.h (Mainnet)
// A more complete list would be needed for full functionality.
const SPORK_2_SWIFTTX_DEFAULT: i64 = 978307200;                         // 2001-1-1 (effectively always on for practical purposes if active)
const SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT_DEFAULT: i64 = 1546731824; // This was PIVX's default, TWINS might differ if they changed spork logic significantly, but usually means OFF by default if high value.
const SPORK_13_ENABLE_SUPERBLOCKS_DEFAULT: i64 = 4070908800;            // Typically OFF by default (future time)
const SPORK_16_ZEROCOIN_MAINTENANCE_MODE_DEFAULT: i64 = 4070908800;     // Typically OFF by default

// Helper to get default value for a spork ID
fn get_default_spork_value(spork_id: i32) -> Option<i64> {
    match spork_id {
        10001 /*SPORK_2_SWIFTTX*/ => Some(SPORK_2_SWIFTTX_DEFAULT),
        10007 /*SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT*/ => Some(SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT_DEFAULT),
        10012 /*SPORK_13_ENABLE_SUPERBLOCKS*/ => Some(SPORK_13_ENABLE_SUPERBLOCKS_DEFAULT),
        10015 /*SPORK_16_ZEROCOIN_MAINTENANCE_MODE*/ => Some(SPORK_16_ZEROCOIN_MAINTENANCE_MODE_DEFAULT),
        // TODO: Add all other spork IDs and their defaults from spork.h
        _ => None, // Spork ID not recognized or has no defined default in this simplified list
    }
}

// Define spork IDs as constants for clarity, mirroring spork.h from C++
// These are just examples, the full list from TWINS-Core/src/spork.h should be used.
// pub const SPORK_2_SWIFTTX: i32 = 10001;
// pub const SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT: i32 = 10007;
// ... etc.

// --- Spork ID Constants (from TWINS-Core/src/spork.h) ---
pub const SPORK_2_SWIFTTX: i32 = 10001;
pub const SPORK_3_SWIFTTX_BLOCK_FILTERING: i32 = 10002;
pub const SPORK_5_MAX_VALUE: i32 = 10004;
pub const SPORK_7_MASTERNODE_SCANNING: i32 = 10006;
pub const SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT: i32 = 10007;
pub const SPORK_9_MASTERNODE_BUDGET_ENFORCEMENT: i32 = 10008;
pub const SPORK_10_MASTERNODE_PAY_UPDATED_NODES: i32 = 10009;
pub const SPORK_13_ENABLE_SUPERBLOCKS: i32 = 10012;
pub const SPORK_14_NEW_PROTOCOL_ENFORCEMENT: i32 = 10013;
pub const SPORK_15_NEW_PROTOCOL_ENFORCEMENT_2: i32 = 10014;
pub const SPORK_16_ZEROCOIN_MAINTENANCE_MODE: i32 = 10015;
// TWINS Specific Sporks (from spork.h)
pub const SPORK_TWINS_01_ENABLE_MASTERNODE_TIERS: i32 = 20190001;
pub const SPORK_TWINS_02_MIN_STAKE_AMOUNT: i32 = 20190002;

// Array of all known spork IDs for iteration
const ALL_SPORK_IDS: [i32; 13] = [
    SPORK_2_SWIFTTX, SPORK_3_SWIFTTX_BLOCK_FILTERING, SPORK_5_MAX_VALUE,
    SPORK_7_MASTERNODE_SCANNING, SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT,
    SPORK_9_MASTERNODE_BUDGET_ENFORCEMENT, SPORK_10_MASTERNODE_PAY_UPDATED_NODES,
    SPORK_13_ENABLE_SUPERBLOCKS, SPORK_14_NEW_PROTOCOL_ENFORCEMENT,
    SPORK_15_NEW_PROTOCOL_ENFORCEMENT_2, SPORK_16_ZEROCOIN_MAINTENANCE_MODE,
    SPORK_TWINS_01_ENABLE_MASTERNODE_TIERS, SPORK_TWINS_02_MIN_STAKE_AMOUNT
];

// --- Default Spork Values (from TWINS-Core/src/spork.h) ---
const SPORK_3_SWIFTTX_BLOCK_FILTERING_DEFAULT: i64 = 1424217600;
const SPORK_5_MAX_VALUE_DEFAULT: i64 = 1000; // This is a value, not a time
const SPORK_7_MASTERNODE_SCANNING_DEFAULT: i64 = 978307200;
const SPORK_9_MASTERNODE_BUDGET_ENFORCEMENT_DEFAULT: i64 = 4070908800; // OFF
const SPORK_10_MASTERNODE_PAY_UPDATED_NODES_DEFAULT: i64 = 4070908800; // OFF
const SPORK_14_NEW_PROTOCOL_ENFORCEMENT_DEFAULT: i64 = 4070908800; // OFF
const SPORK_15_NEW_PROTOCOL_ENFORCEMENT_2_DEFAULT: i64 = 4070908800; // OFF
const SPORK_TWINS_01_ENABLE_MASTERNODE_TIERS_DEFAULT: i64 = 4070908800; // OFF
const SPORK_TWINS_02_MIN_STAKE_AMOUNT_DEFAULT: i64 = 4070908800; // OFF (Value is a min amount, not timestamp)

// Spork public keys and activation times (already defined)
// ...

#[derive(Debug, Serialize, Clone)] // Added Serialize and Clone
pub struct SporkDetail {
    pub id: i32,
    pub name: String,
    pub value: Option<i64>,
    pub active: bool,
}

#[derive(Debug)]
pub struct SporkManager {
    // Using RwLock for concurrent reads of spork values.
    active_sporks: RwLock<HashMap<i32, SporkMessage>>,
    storage: Arc<dyn BlockStorage>,
    secp: Secp256k1<secp256k1::VerifyOnly>,
    // TODO: Potentially add CSporkManager equivalent fields if complex signing/verification logic is needed here.
    // For now, assuming sporks are received, validated (basic signature check), and stored.
}

impl SporkManager {
    pub fn new(storage: Arc<dyn BlockStorage>) -> Self {
        // TODO: Load sporks from storage on startup
        let loaded_sporks = match storage.load_sporks() {
            Ok(sporks) => sporks,
            Err(e) => {
                log::error!("Failed to load sporks from DB, starting with empty set: {}", e);
                HashMap::new()
            }
        };
        SporkManager {
            active_sporks: RwLock::new(loaded_sporks),
            storage,
            secp: Secp256k1::verification_only(),
        }
    }

    fn get_spork_message_hash(spork_msg: &SporkMessage) -> Result<[u8; 32], std::io::Error> {
        let mut writer = Vec::new();
        writer.write_i32::<LittleEndian>(spork_msg.n_spork_id)?;
        writer.write_i64::<LittleEndian>(spork_msg.n_value)?;
        writer.write_i64::<LittleEndian>(spork_msg.n_time_signed)?;
        
        let first_hash = Sha256::digest(&writer);
        let second_hash = Sha256::digest(&first_hash);
        Ok(second_hash.into())
    }

    // Processes a spork message received from a peer
    pub fn process_spork_message(&self, spork_msg: SporkMessage, _source_peer_addr: Option<SocketAddr>) {
        log::debug!(
            "Attempting to process Spork ID: {}, Value: {}, TimeSigned: {}",
            spork_msg.n_spork_id,
            spork_msg.n_value,
            spork_msg.n_time_signed
        );

        // Determine which key to use for verification
        let spork_pub_key_hex_str = 
            if spork_msg.n_time_signed >= MAINNET_REJECT_OLD_SPORK_KEY {
                MAINNET_SPORK_KEY_NEW // Must use new key
            } else if spork_msg.n_time_signed >= MAINNET_ENFORCE_NEW_SPORK_KEY {
                MAINNET_SPORK_KEY_NEW // New key preferred and enforced
            } else {
                MAINNET_SPORK_KEY_OLD // Old key is still acceptable
            };

        let pubkey_bytes = match hex::decode(spork_pub_key_hex_str) {
            Ok(bytes) => bytes,
            Err(e) => {
                log::error!("Failed to decode spork public key hex {}: {}", spork_pub_key_hex_str, e);
                return;
            }
        };

        let public_key = match PublicKey::from_slice(&pubkey_bytes) {
            Ok(key) => key,
            Err(e) => {
                log::error!("Failed to parse spork public key for spork ID {}: {}", spork_msg.n_spork_id, e);
                return;
            }
        };

        let signature = match Signature::from_der(&spork_msg.vch_sig) {
            Ok(sig) => sig,
            Err(e) => {
                log::error!("Failed to parse DER signature for spork ID {}: {}", spork_msg.n_spork_id, e);
                return;
            }
        };

        match Self::get_spork_message_hash(&spork_msg) {
            Ok(msg_hash_bytes) => {
                let message_to_verify = match SecpMessage::from_slice(&msg_hash_bytes) {
                    Ok(msg) => msg,
                    Err(e) => {
                        log::error!("Failed to create SecpMessage from hash for spork ID {}: {}", spork_msg.n_spork_id, e);
                        return;
                    }
                };

                if self.secp.verify_ecdsa(&message_to_verify, &signature, &public_key).is_ok() {
                    log::info!("Spork ID {} signature VERIFIED. Value: {}, TimeSigned: {}", 
                        spork_msg.n_spork_id, spork_msg.n_value, spork_msg.n_time_signed);
                    
                    let mut sporks_map = self.active_sporks.write().unwrap();
                    if let Some(existing_spork) = sporks_map.get(&spork_msg.n_spork_id) {
                        if spork_msg.n_time_signed > existing_spork.n_time_signed {
                            log::info!("Updating existing Spork ID: {}. New TimeSigned: {}, Old TimeSigned: {}", 
                                spork_msg.n_spork_id, spork_msg.n_time_signed, existing_spork.n_time_signed);
                            if let Err(e) = self.storage.save_spork(&spork_msg) {
                                log::error!("Failed to save updated spork {} to DB: {}", spork_msg.n_spork_id, e);
                            }
                            sporks_map.insert(spork_msg.n_spork_id, spork_msg);
                        } else {
                            log::debug!("Received older or same Spork ID: {}. Ignoring.", spork_msg.n_spork_id);
                        }
                    } else {
                        log::info!("Adding new Spork ID: {}. Value: {}.", spork_msg.n_spork_id, spork_msg.n_value);
                        if let Err(e) = self.storage.save_spork(&spork_msg) {
                            log::error!("Failed to save new spork {} to DB: {}", spork_msg.n_spork_id, e);
                        }
                        sporks_map.insert(spork_msg.n_spork_id, spork_msg);
                    }
                } else {
                    log::warn!(
                        "Spork ID {} signature INVALID. Discarding. Value: {}, TimeSigned: {}, PubKey Used: {}",
                        spork_msg.n_spork_id, spork_msg.n_value, spork_msg.n_time_signed, spork_pub_key_hex_str
                    );
                }
            },
            Err(e) => {
                 log::error!("Failed to get message hash for spork ID {}: {}", spork_msg.n_spork_id, e);
            }
        }
        // TODO: Relay valid new/updated sporks to other peers (if this node is a source)
    }

    // Checks if a specific spork is active (its value is less than current time)
    pub fn is_spork_active(&self, spork_id: i32) -> bool {
        let sporks_map = self.active_sporks.read().unwrap();
        let current_time = chrono::Utc::now().timestamp();

        let value_to_check = match sporks_map.get(&spork_id) {
            Some(spork) => spork.n_value,
            None => {
                // Spork not in active map, use its default value
                match get_default_spork_value(spork_id) {
                    Some(default_val) => default_val,
                    None => {
                        log::warn!("Spork ID {} not found in active_sporks and no default configured. Assuming inactive.", spork_id);
                        return false; // Or a specific behavior for unknown sporks
                    }
                }
            }
        };

        // Special handling for sporks that are not time-based (e.g., SPORK_5_MAX_VALUE)
        // This is a simplified check; actual C++ logic might be more nuanced per spork.
        if spork_id == SPORK_5_MAX_VALUE || spork_id == SPORK_TWINS_02_MIN_STAKE_AMOUNT {
            // These are not time-based enable/disable flags in the same way.
            // Their 'value' is the actual setting. We consider them "active" if a value is set.
            // The actual enforcement happens where this value is read.
            // If relying on default, it's "active" in the sense that the default value applies.
            return true; // The spork value itself is the setting.
        }

        // PIVX/Dash spork logic: if value > 4000000000 (approx), it's usually considered off.
        // Otherwise, it's active if its value (timestamp) is less than the current time.
        if value_to_check > 4000000000 {
            false
        } else {
            value_to_check < current_time
        }
    }

    // Gets the value of a specific spork
    pub fn get_spork_value(&self, spork_id: i32) -> Option<i64> {
        let sporks_map = self.active_sporks.read().unwrap();
        match sporks_map.get(&spork_id) {
            Some(spork) => Some(spork.n_value),
            None => get_default_spork_value(spork_id), // Fallback to default
        }
    }

    // New methods for API
    pub fn get_spork_name_by_id(&self, id: i32) -> String {
        match id {
            SPORK_2_SWIFTTX => "SPORK_2_SWIFTTX".to_string(),
            SPORK_3_SWIFTTX_BLOCK_FILTERING => "SPORK_3_SWIFTTX_BLOCK_FILTERING".to_string(),
            SPORK_5_MAX_VALUE => "SPORK_5_MAX_VALUE".to_string(),
            SPORK_7_MASTERNODE_SCANNING => "SPORK_7_MASTERNODE_SCANNING".to_string(),
            SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT => "SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT".to_string(),
            SPORK_9_MASTERNODE_BUDGET_ENFORCEMENT => "SPORK_9_MASTERNODE_BUDGET_ENFORCEMENT".to_string(),
            SPORK_10_MASTERNODE_PAY_UPDATED_NODES => "SPORK_10_MASTERNODE_PAY_UPDATED_NODES".to_string(),
            SPORK_13_ENABLE_SUPERBLOCKS => "SPORK_13_ENABLE_SUPERBLOCKS".to_string(),
            SPORK_14_NEW_PROTOCOL_ENFORCEMENT => "SPORK_14_NEW_PROTOCOL_ENFORCEMENT".to_string(),
            SPORK_15_NEW_PROTOCOL_ENFORCEMENT_2 => "SPORK_15_NEW_PROTOCOL_ENFORCEMENT_2".to_string(),
            SPORK_16_ZEROCOIN_MAINTENANCE_MODE => "SPORK_16_ZEROCOIN_MAINTENANCE_MODE".to_string(),
            SPORK_TWINS_01_ENABLE_MASTERNODE_TIERS => "SPORK_TWINS_01_ENABLE_MASTERNODE_TIERS".to_string(),
            SPORK_TWINS_02_MIN_STAKE_AMOUNT => "SPORK_TWINS_02_MIN_STAKE_AMOUNT".to_string(),
            _ => format!("UNKNOWN_SPORK_{}", id),
        }
    }

    pub fn get_all_spork_details(&self) -> Vec<SporkDetail> {
        let mut details = Vec::new();
        for &spork_id in ALL_SPORK_IDS.iter() {
            details.push(SporkDetail {
                id: spork_id,
                name: self.get_spork_name_by_id(spork_id),
                value: self.get_spork_value(spork_id),
                active: self.is_spork_active(spork_id),
            });
        }
        details
    }
} 