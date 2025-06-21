use crate::p2p::messages::{
    BlockHeaderData, BlockMessage, SporkMessage, MasternodeBroadcast,
    BudgetProposalBroadcast, BudgetVote, FinalizedBudgetBroadcast, FinalizedBudgetVote,
    TxLockRequest, ConsensusVote, TransactionData, Encodable, Decodable
};
use crate::blockchain::block_index::BlockIndex;
use crate::masternode_manager::MasternodeOutPoint;
use crate::governance_manager::{ProposalHash, BudgetHash};
use crate::instant_send_manager::TxId;
use rusqlite::{Connection, Result, params, Error as RusqliteError};
use std::sync::{Mutex};
use std::collections::HashMap;

pub trait BlockStorage: Send + Sync + std::fmt::Debug {
    fn save_header(&self, block_index: &BlockIndex) -> Result<(), RusqliteError>;
    fn get_header(&self, hash: &[u8; 32]) -> Result<Option<BlockIndex>, RusqliteError>;
    fn save_block(&self, block_message: &BlockMessage) -> Result<(), RusqliteError>;
    fn get_block(&self, hash: &[u8; 32]) -> Result<Option<BlockMessage>, RusqliteError>;
    fn has_block(&self, hash: &[u8; 32]) -> Result<bool, RusqliteError>;
    fn get_chain_tip_hash(&self) -> Result<Option<[u8; 32]>, RusqliteError>;
    fn set_chain_tip_hash(&self, hash: &[u8; 32]) -> Result<(), RusqliteError>;
    fn save_spork(&self, spork: &SporkMessage) -> Result<(), RusqliteError>;
    fn load_sporks(&self) -> Result<HashMap<i32, SporkMessage>, RusqliteError>;
    fn save_masternode(&self, outpoint: &MasternodeOutPoint, mnb: &MasternodeBroadcast) -> Result<(), RusqliteError>;
    fn load_masternodes(&self) -> Result<HashMap<MasternodeOutPoint, MasternodeBroadcast>, RusqliteError>;
    fn remove_masternode(&self, outpoint: &MasternodeOutPoint) -> Result<(), RusqliteError>;
    fn save_budget_proposal(&self, proposal_hash: &ProposalHash, proposal: &BudgetProposalBroadcast) -> Result<(), RusqliteError>;
    fn save_budget_vote(&self, vote_hash: &[u8;32], vote: &BudgetVote) -> Result<(), RusqliteError>;
    fn save_finalized_budget(&self, budget_hash: &BudgetHash, fbs: &FinalizedBudgetBroadcast) -> Result<(), RusqliteError>;
    fn save_finalized_budget_vote(&self, vote_hash: &[u8;32], fbvote: &FinalizedBudgetVote) -> Result<(), RusqliteError>;
    fn load_governance_objects(&self) -> Result<(
        HashMap<ProposalHash, BudgetProposalBroadcast>,
        HashMap<BudgetHash, FinalizedBudgetBroadcast>,
        HashMap<[u8;32], BudgetVote>,
        HashMap<[u8;32], FinalizedBudgetVote>
    ), RusqliteError>;
    fn save_tx_lock_request(&self, txid: &TxId, request: &TxLockRequest) -> Result<(), RusqliteError>;
    fn save_consensus_vote(&self, txid: &TxId, vote_hash: &[u8;32], vote: &ConsensusVote) -> Result<(), RusqliteError>;
    fn load_instantsend_objects(&self) -> Result<(
        HashMap<TxId, TxLockRequest>,
        HashMap<TxId, Vec<ConsensusVote>>
    ), RusqliteError>;
    fn get_block_hash_by_height(&self, height: u32) -> Result<Option<[u8; 32]>, RusqliteError>;
    fn get_transaction_details(&self, txid: &[u8; 32]) -> Result<Option<(TransactionData, Option<[u8;32]>, Option<u32>)>, RusqliteError>;
    fn get_block_hashes_by_height_range(&self, max_height: u32, count: u32) -> Result<Vec<[u8; 32]>, RusqliteError>;
}

pub struct SqliteBlockStorage {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteBlockStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteBlockStorage")
         .field("conn", &"Mutex<Connection>") 
         .finish()
    }
}

impl SqliteBlockStorage {
    pub fn new(db_path: &str) -> Result<Self, RusqliteError> {
        let conn = Connection::open(db_path)?;
        conn.execute("CREATE TABLE IF NOT EXISTS block_headers (hash BLOB PRIMARY KEY, height INTEGER NOT NULL, prev_hash BLOB, header_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS blocks (hash BLOB PRIMARY KEY, block_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS chain_metadata (key TEXT PRIMARY KEY, value_blob BLOB, value_text TEXT)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS sporks (spork_id INTEGER PRIMARY KEY, value INTEGER NOT NULL, time_signed INTEGER NOT NULL, signature BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS masternodes (collateral_hash TEXT NOT NULL, collateral_index INTEGER NOT NULL, mnb_data BLOB NOT NULL, PRIMARY KEY (collateral_hash, collateral_index))", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS budget_proposals (proposal_hash BLOB PRIMARY KEY, proposal_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS budget_votes (vote_hash BLOB PRIMARY KEY, vote_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS finalized_budgets (budget_hash BLOB PRIMARY KEY, budget_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS finalized_budget_votes (vote_hash BLOB PRIMARY KEY, vote_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS tx_lock_requests (txid BLOB PRIMARY KEY, request_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS tx_lock_votes (vote_hash BLOB PRIMARY KEY, txid BLOB NOT NULL, vote_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_tx_lock_votes_txid ON tx_lock_votes (txid);", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS transactions (txid BLOB PRIMARY KEY, block_hash BLOB, block_height INTEGER, tx_data BLOB NOT NULL)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_transactions_block_hash ON transactions (block_hash);", [])?;
        Ok(SqliteBlockStorage { conn: Mutex::new(conn) })
    }
}

impl BlockStorage for SqliteBlockStorage {
    fn save_header(&self, block_index: &BlockIndex) -> Result<(), RusqliteError> { 
        let conn = self.conn.lock().unwrap(); 
        let mut header_data_bytes = Vec::new();
        block_index.header.consensus_encode(&mut std::io::Cursor::new(&mut header_data_bytes))
            .map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO block_headers (hash, height, prev_hash, header_data) VALUES (?1, ?2, ?3, ?4)",
            params![
                block_index.hash.to_vec(),
                block_index.height,
                block_index.prev_hash.map(|h| h.to_vec()),
                header_data_bytes
            ],
        )?;
        Ok(())
    }
    fn get_header(&self, hash: &[u8; 32]) -> Result<Option<BlockIndex>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT height, prev_hash, header_data FROM block_headers WHERE hash = ?1")?;
        let mut rows = stmt.query(params![hash.to_vec()])?;
        if let Some(row) = rows.next()? {
            let height: u32 = row.get(0)?;
            let prev_hash_opt_vec: Option<Vec<u8>> = row.get(1)?;
            let header_data_bytes: Vec<u8> = row.get(2)?;
            let prev_hash_opt: Option<[u8; 32]> = prev_hash_opt_vec.map(|v| {
                let mut arr = [0u8; 32]; arr.copy_from_slice(&v); arr
            });
            let header_data = BlockHeaderData::consensus_decode(&mut std::io::Cursor::new(header_data_bytes))
                .map_err(|e| RusqliteError::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok(Some(BlockIndex { hash: *hash, prev_hash: prev_hash_opt, height, header: header_data, chain_work_proxy: height as u64 }))
        } else { Ok(None) }
    }

    fn save_block(&self, block_message: &BlockMessage) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let block_hash = block_message.header.get_hash();
        let mut block_data_bytes = Vec::new();
        block_message.consensus_encode(&mut std::io::Cursor::new(&mut block_data_bytes))
            .map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO blocks (hash, block_data) VALUES (?1, ?2)",
            params![block_hash.to_vec(), block_data_bytes],
        )?;
        let stored_header_height: Option<u32> = conn.query_row(
            "SELECT height FROM block_headers WHERE hash = ?1", 
            params![block_hash.to_vec()], 
            |row| row.get(0)
        ).ok(); 
        for tx_data in &block_message.transactions {
            let txid = tx_data.get_txid();
            let mut tx_bytes = Vec::new();
            tx_data.consensus_encode(&mut std::io::Cursor::new(&mut tx_bytes))
                .map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
            conn.execute(
                "INSERT OR REPLACE INTO transactions (txid, block_hash, block_height, tx_data) VALUES (?1, ?2, ?3, ?4)",
                params![txid.to_vec(), Some(block_hash.to_vec()), stored_header_height, tx_bytes],
            )?;
        }
        Ok(())
    }
    fn get_block(&self, hash: &[u8; 32]) -> Result<Option<BlockMessage>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT block_data FROM blocks WHERE hash = ?1")?;
        let mut rows = stmt.query(params![hash.to_vec()])?;
        if let Some(row) = rows.next()? {
            let block_data_bytes: Vec<u8> = row.get(0)?;
            let block_message = BlockMessage::consensus_decode(&mut std::io::Cursor::new(block_data_bytes))
                 .map_err(|e| RusqliteError::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok(Some(block_message))
        } else { Ok(None) }
    }
    
    fn has_block(&self, hash: &[u8; 32]) -> Result<bool, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM blocks WHERE hash = ?1",
            params![hash.to_vec()],
            |row| row.get(0)
        )?;
        Ok(count > 0)
    }
    fn get_chain_tip_hash(&self) -> Result<Option<[u8; 32]>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value_blob FROM chain_metadata WHERE key = 'chain_tip_hash'")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let hash_vec: Option<Vec<u8>> = row.get(0)?;
            Ok(hash_vec.map(|v| { let mut arr = [0u8; 32]; arr.copy_from_slice(&v); arr }))
        } else {
            Ok(None)
        }
    }
    fn set_chain_tip_hash(&self, hash: &[u8; 32]) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT OR REPLACE INTO chain_metadata (key, value_blob) VALUES ('chain_tip_hash', ?1)", params![hash.to_vec()])?;
        Ok(())
    }
    fn save_spork(&self, spork: &SporkMessage) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT OR REPLACE INTO sporks (spork_id, value, time_signed, signature) VALUES (?1, ?2, ?3, ?4)",
            params![spork.n_spork_id, spork.n_value, spork.n_time_signed, spork.vch_sig.clone()])?;
        Ok(())
    }
    fn load_sporks(&self) -> Result<HashMap<i32, SporkMessage>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT spork_id, value, time_signed, signature FROM sporks")?;
        let spork_iter = stmt.query_map([], |row| {
            Ok(SporkMessage { n_spork_id: row.get(0)?, n_value: row.get(1)?, n_time_signed: row.get(2)?, vch_sig: row.get(3)? })
        })?;
        let mut sporks_map = HashMap::new();
        for spork_result in spork_iter { let spork = spork_result?; sporks_map.insert(spork.n_spork_id, spork); }
        Ok(sporks_map)
    }
    fn save_masternode(&self, outpoint: &MasternodeOutPoint, mnb: &MasternodeBroadcast) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut mnb_data_bytes = Vec::new();
        mnb.consensus_encode(&mut std::io::Cursor::new(&mut mnb_data_bytes)).map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT OR REPLACE INTO masternodes (collateral_hash, collateral_index, mnb_data) VALUES (?1, ?2, ?3)",
            params![hex::encode(outpoint.0), outpoint.1, mnb_data_bytes])?;
        Ok(())
    }
    fn load_masternodes(&self) -> Result<HashMap<MasternodeOutPoint, MasternodeBroadcast>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT collateral_hash, collateral_index, mnb_data FROM masternodes")?;
        let mn_iter = stmt.query_map([], |row| {
            let collateral_hash_hex: String = row.get(0)?;
            let collateral_index: u32 = row.get(1)?;
            let mnb_data_bytes: Vec<u8> = row.get(2)?;
            let mut collateral_hash = [0u8; 32];
            hex::decode_to_slice(&collateral_hash_hex, &mut collateral_hash).map_err(|e| RusqliteError::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
            let mnb = MasternodeBroadcast::consensus_decode(&mut std::io::Cursor::new(mnb_data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(2, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok(((collateral_hash, collateral_index), mnb))
        })?;
        let mut mn_map = HashMap::new();
        for mn_result in mn_iter { let (outpoint, mnb) = mn_result?; mn_map.insert(outpoint, mnb); }
        Ok(mn_map)
    }
    fn remove_masternode(&self, outpoint: &MasternodeOutPoint) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM masternodes WHERE collateral_hash = ?1 AND collateral_index = ?2", params![hex::encode(outpoint.0), outpoint.1])?;
        Ok(())
    }
    fn save_budget_proposal(&self, proposal_hash: &ProposalHash, proposal: &BudgetProposalBroadcast) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut data_bytes = Vec::new(); proposal.consensus_encode(&mut std::io::Cursor::new(&mut data_bytes)).map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT OR REPLACE INTO budget_proposals (proposal_hash, proposal_data) VALUES (?1, ?2)", params![proposal_hash.to_vec(), data_bytes])?;
        Ok(())
    }
    fn save_budget_vote(&self, vote_hash: &[u8;32], vote: &BudgetVote) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut data_bytes = Vec::new(); vote.consensus_encode(&mut std::io::Cursor::new(&mut data_bytes)).map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT OR REPLACE INTO budget_votes (vote_hash, vote_data) VALUES (?1, ?2)", params![vote_hash.to_vec(), data_bytes])?;
        Ok(())
    }
    fn save_finalized_budget(&self, budget_hash: &BudgetHash, fbs: &FinalizedBudgetBroadcast) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut data_bytes = Vec::new(); fbs.consensus_encode(&mut std::io::Cursor::new(&mut data_bytes)).map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT OR REPLACE INTO finalized_budgets (budget_hash, budget_data) VALUES (?1, ?2)", params![budget_hash.to_vec(), data_bytes])?;
        Ok(())
    }
    fn save_finalized_budget_vote(&self, vote_hash: &[u8;32], fbvote: &FinalizedBudgetVote) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut data_bytes = Vec::new(); fbvote.consensus_encode(&mut std::io::Cursor::new(&mut data_bytes)).map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT OR REPLACE INTO finalized_budget_votes (vote_hash, vote_data) VALUES (?1, ?2)", params![vote_hash.to_vec(), data_bytes])?;
        Ok(())
    }
    fn load_governance_objects(&self) -> Result<(
        HashMap<ProposalHash, BudgetProposalBroadcast>,
        HashMap<BudgetHash, FinalizedBudgetBroadcast>,
        HashMap<[u8;32], BudgetVote>,
        HashMap<[u8;32], FinalizedBudgetVote>
    ), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut proposals_map = HashMap::new(); 
        let mut stmt_props = conn.prepare("SELECT proposal_hash, proposal_data FROM budget_proposals")?;
        let prop_iter = stmt_props.query_map([], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let data_bytes: Vec<u8> = row.get(1)?;
            let mut hash = [0u8; 32]; hash.copy_from_slice(&hash_bytes);
            let prop = BudgetProposalBroadcast::consensus_decode(&mut std::io::Cursor::new(data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(1, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok((hash, prop))
        })?;
        for res in prop_iter { let (hash, prop) = res?; proposals_map.insert(hash, prop); }

        let mut final_budgets_map = HashMap::new();
        let mut stmt_fbs = conn.prepare("SELECT budget_hash, budget_data FROM finalized_budgets")?;
        let fbs_iter = stmt_fbs.query_map([], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let data_bytes: Vec<u8> = row.get(1)?;
            let mut hash = [0u8; 32]; hash.copy_from_slice(&hash_bytes);
            let fbs = FinalizedBudgetBroadcast::consensus_decode(&mut std::io::Cursor::new(data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(1, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok((hash, fbs))
        })?;
        for res in fbs_iter { let (hash, fbs) = res?; final_budgets_map.insert(hash, fbs); }

        let mut budget_votes_map = HashMap::new();
        let mut stmt_votes = conn.prepare("SELECT vote_hash, vote_data FROM budget_votes")?;
        let vote_iter = stmt_votes.query_map([], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let data_bytes: Vec<u8> = row.get(1)?;
            let mut hash = [0u8; 32]; hash.copy_from_slice(&hash_bytes);
            let vote = BudgetVote::consensus_decode(&mut std::io::Cursor::new(data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(1, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok((hash, vote))
        })?;
        for res in vote_iter { let (hash, vote) = res?; budget_votes_map.insert(hash, vote); }

        let mut fb_votes_map = HashMap::new();
        let mut stmt_fbvotes = conn.prepare("SELECT vote_hash, vote_data FROM finalized_budget_votes")?;
        let fbvote_iter = stmt_fbvotes.query_map([], |row| {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let data_bytes: Vec<u8> = row.get(1)?;
            let mut hash = [0u8; 32]; hash.copy_from_slice(&hash_bytes);
            let fbvote = FinalizedBudgetVote::consensus_decode(&mut std::io::Cursor::new(data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(1, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok((hash, fbvote))
        })?;
        for res in fbvote_iter { let (hash, fbvote) = res?; fb_votes_map.insert(hash, fbvote); }
        
        Ok((proposals_map, final_budgets_map, budget_votes_map, fb_votes_map))
    }
    fn save_tx_lock_request(&self, txid: &TxId, request: &TxLockRequest) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut data_bytes = Vec::new(); request.consensus_encode(&mut std::io::Cursor::new(&mut data_bytes)).map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT OR REPLACE INTO tx_lock_requests (txid, request_data) VALUES (?1, ?2)", params![txid.to_vec(), data_bytes])?;
        Ok(())
    }
    fn save_consensus_vote(&self, txid: &TxId, vote_hash: &[u8;32], vote: &ConsensusVote) -> Result<(), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut data_bytes = Vec::new(); vote.consensus_encode(&mut std::io::Cursor::new(&mut data_bytes)).map_err(|e| RusqliteError::ToSqlConversionFailure(Box::new(e)))?;
        conn.execute("INSERT OR REPLACE INTO tx_lock_votes (vote_hash, txid, vote_data) VALUES (?1, ?2, ?3)", params![vote_hash.to_vec(), txid.to_vec(), data_bytes])?;
        Ok(())
    }
    fn load_instantsend_objects(&self) -> Result<(HashMap<TxId, TxLockRequest>, HashMap<TxId, Vec<ConsensusVote>>), RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut requests_map = HashMap::new();
        let mut stmt_reqs = conn.prepare("SELECT txid, request_data FROM tx_lock_requests")?;
        let req_iter = stmt_reqs.query_map([], |row| {
            let txid_bytes: Vec<u8> = row.get(0)?;
            let data_bytes: Vec<u8> = row.get(1)?;
            let mut txid_arr = [0u8; 32]; txid_arr.copy_from_slice(&txid_bytes);
            let req = TxLockRequest::consensus_decode(&mut std::io::Cursor::new(data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(1, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok((txid_arr, req))
        })?;
        for res in req_iter { let (txid_arr, req) = res?; requests_map.insert(txid_arr, req); }

        let mut votes_map_grouped: HashMap<TxId, Vec<ConsensusVote>> = HashMap::new();
        let mut stmt_votes = conn.prepare("SELECT txid, vote_data FROM tx_lock_votes ORDER BY txid")?;
        let vote_iter = stmt_votes.query_map([], |row| {
            let txid_bytes: Vec<u8> = row.get(0)?;
            let data_bytes: Vec<u8> = row.get(1)?;
            let mut txid_arr = [0u8; 32]; txid_arr.copy_from_slice(&txid_bytes);
            let vote = ConsensusVote::consensus_decode(&mut std::io::Cursor::new(data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(1, rusqlite::types::Type::Blob, Box::new(e)))?;
            Ok((txid_arr, vote))
        })?;
        for res in vote_iter { let (txid_arr, vote) = res?; votes_map_grouped.entry(txid_arr).or_default().push(vote); }
        
        Ok((requests_map, votes_map_grouped))
    }
    fn get_block_hash_by_height(&self, height: u32) -> Result<Option<[u8; 32]>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT hash FROM block_headers WHERE height = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![height])?;
        if let Some(row) = rows.next()? {
            let hash_vec: Vec<u8> = row.get(0)?;
            if hash_vec.len() == 32 { let mut arr = [0u8; 32]; arr.copy_from_slice(&hash_vec); Ok(Some(arr)) }
            else { Err(RusqliteError::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid hash length")))) }
        } else { Ok(None) }
    }

    fn get_transaction_details(&self, txid: &[u8; 32]) -> Result<Option<(TransactionData, Option<[u8;32]>, Option<u32>)>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT tx_data, block_hash, block_height FROM transactions WHERE txid = ?1")?;
        let mut rows = stmt.query(params![txid.to_vec()])?;
        if let Some(row) = rows.next()? {
            let tx_data_bytes: Vec<u8> = row.get(0)?;
            let block_hash_opt_vec: Option<Vec<u8>> = row.get(1)?;
            let block_height_opt: Option<u32> = row.get(2)?;
            let tx_data = TransactionData::consensus_decode(&mut std::io::Cursor::new(tx_data_bytes)).map_err(|e| RusqliteError::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e)))?;
            let block_hash_opt: Option<[u8;32]> = block_hash_opt_vec.map(|v| { let mut arr = [0u8; 32]; arr.copy_from_slice(&v); arr });
            Ok(Some((tx_data, block_hash_opt, block_height_opt)))
        } else { Ok(None) }
    }
    fn get_block_hashes_by_height_range(&self, max_height: u32, count: u32) -> Result<Vec<[u8; 32]>, RusqliteError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT hash FROM block_headers WHERE height <= ?1 ORDER BY height DESC LIMIT ?2")?;
        let rows = stmt.query_map(params![max_height, count], |row| {
            let hash_vec: Vec<u8> = row.get(0)?;
            if hash_vec.len() == 32 {
                let mut hash_array = [0u8; 32];
                hash_array.copy_from_slice(&hash_vec);
                Ok(hash_array)
            } else {
                Err(RusqliteError::FromSqlConversionFailure(
                    0, 
                    rusqlite::types::Type::Blob, 
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid hash length in DB for get_block_hashes_by_height_range"))
                ))
            }
        })?;
        let mut hashes = Vec::new();
        for row_result in rows { hashes.push(row_result?); }
        Ok(hashes)
    }
}