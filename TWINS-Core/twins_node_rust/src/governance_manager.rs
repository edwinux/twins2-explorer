use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::net::SocketAddr;

use crate::p2p::messages::{
    BudgetProposalBroadcast, BudgetVote, FinalizedBudgetBroadcast, FinalizedBudgetVote,
    MnvsMessage, CMD_MNVS, MessageHeader, Encodable
};
use crate::storage::BlockStorage;
use crate::spork_manager::SporkManager; // To check sporks like SPORK_9_MASTERNODE_BUDGET_ENFORCEMENT
use crate::p2p::connection; // For send_message, calculate_checksum
use tokio::net::TcpStream; // For the stream in request_budget_sync
use serde::Serialize; // For ApiBudgetProposalSummary

// Define types for keys if needed, e.g., proposal hash, budget hash
pub type ProposalHash = [u8; 32];
pub type BudgetHash = [u8; 32];

#[derive(Debug, Serialize, Clone)] // For API responses
#[serde(rename_all = "camelCase")]
pub struct ApiBudgetProposalSummary {
    pub proposal_hash: String,
    pub name: String,
    pub url: String,
    pub block_start: i32,
    pub block_end: i32,
    pub total_amount: String, // decimal string
    pub monthly_amount: String, // decimal string (placeholder calculation for now)
    pub payee_address: String, // TWINS address
    pub fee_tx_hash: String,
    pub time_created: i64, // unix timestamp
    pub yeas: i32, // Placeholder
    pub nays: i32, // Placeholder
    pub abstains: i32, // Placeholder
    pub is_established: bool, // Placeholder
    pub is_passing: bool, // Placeholder
    pub status: String, // e.g., "pending", "passing", "failing", "funded"
}

#[derive(Debug)]
pub struct GovernanceManager {
    proposals: RwLock<HashMap<ProposalHash, BudgetProposalBroadcast>>,
    // Votes are typically associated with a proposal, so perhaps a nested map or a more complex structure
    // For now, let's assume votes are processed and maybe stored if valid, but not deeply managed here yet.
    // Same for finalized budgets and their votes.
    finalized_budgets: RwLock<HashMap<BudgetHash, FinalizedBudgetBroadcast>>,

    storage: Arc<dyn BlockStorage>,
    spork_manager: Arc<SporkManager>,
    // TODO: Add fields for tracking sync state, votes per proposal, etc.
}

impl GovernanceManager {
    pub fn new(storage: Arc<dyn BlockStorage>, spork_manager: Arc<SporkManager>) -> Self {
        // TODO: Load proposals, votes, finalized budgets from storage on startup
        let (loaded_proposals, loaded_finalized_budgets, _loaded_budget_votes, _loaded_fb_votes) = 
            match storage.load_governance_objects() { // Assuming a new combined load function
                Ok((props, fin_buds, budget_votes, fb_votes)) => (props, fin_buds, budget_votes, fb_votes),
                Err(e) => {
                    log::error!("Failed to load governance objects from DB, starting empty: {}", e);
                    (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new())
                }
            };

        GovernanceManager {
            proposals: RwLock::new(loaded_proposals),
            finalized_budgets: RwLock::new(loaded_finalized_budgets),
            storage,
            spork_manager,
        }
    }

    // Processes a BudgetProposalBroadcast message
    pub fn process_mprop_message(&self, proposal: BudgetProposalBroadcast, _source_peer_addr: Option<SocketAddr>) {
        log::info!("Processing MPROP: Name='{}', URL='{}'", proposal.str_proposal_name, proposal.str_url);
        // TODO: Implement CBudgetProposal::IsValid equivalent validation (fee tx, signature, etc.)
        // TODO: Store valid proposal in self.proposals and self.storage.save_budget_proposal()
        // TODO: Relay valid new proposals
    }

    // Processes a BudgetVote message
    pub fn process_mvote_message(&self, vote: BudgetVote, _source_peer_addr: Option<SocketAddr>) {
        log::info!("Processing MVOTE: ProposalHash={}, Vote={}", hex::encode(vote.n_proposal_hash), vote.n_vote);
        // TODO: Implement CBudgetVote::SignatureValid and other checks.
        // TODO: Find proposal by hash, add vote to it, update storage (self.storage.save_budget_vote, or update proposal in DB).
        // TODO: Relay valid new votes.
    }

    // Processes a FinalizedBudgetBroadcast message
    pub fn process_fbs_message(&self, fbs: FinalizedBudgetBroadcast, _source_peer_addr: Option<SocketAddr>) {
        log::info!("Processing FBS: Name='{}', BlockStart={}", fbs.str_budget_name, fbs.n_block_start);
        // TODO: Implement CFinalizedBudget::IsValid (fee tx, etc.)
        // TODO: Store valid finalized budget in self.finalized_budgets and self.storage.save_finalized_budget()
        // TODO: Relay valid new finalized budgets
    }

    // Processes a FinalizedBudgetVote message
    pub fn process_fbvote_message(&self, vote: FinalizedBudgetVote, _source_peer_addr: Option<SocketAddr>) {
        log::info!("Processing FBVOTE: BudgetHash={}", hex::encode(vote.n_budget_hash));
        // TODO: Implement CFinalizedBudgetVote::SignatureValid and other checks.
        // TODO: Find finalized budget by hash, add vote, update storage.
        // TODO: Relay valid new votes.
    }

    // Requests budget synchronization from a peer
    pub async fn request_budget_sync(&self, stream: &mut TcpStream) -> Result<(), std::io::Error> {
        log::debug!("GovernanceManager: Sending MNVS (budget sync request) message");
        let mnvs_payload_data = MnvsMessage { hash: [0u8; 32] }; // Null hash for full sync
        let mut mnvs_payload_bytes = Vec::new();
        mnvs_payload_data.consensus_encode(&mut std::io::Cursor::new(&mut mnvs_payload_bytes))?;
        let checksum = connection::calculate_checksum(&mnvs_payload_bytes);
        let header = MessageHeader::new(*CMD_MNVS, mnvs_payload_bytes.len() as u32, checksum);
        connection::send_message(stream, header, &mnvs_payload_bytes).await
        // TODO: Manage request state for budget sync
    }
    
    // TODO: Add other methods like: 
    // get_proposal_by_hash, get_finalized_budget_by_hash, 
    // get_total_budget_for_block, is_payment_block, fill_block_payee (for block creation if node stakes/mines)

    // New method for API
    pub fn get_proposals_summary_list(
        &self,
        _page: u32, // Pagination to be fully implemented later
        _limit: u32,
    ) -> (Vec<ApiBudgetProposalSummary>, usize) {
        let proposals_map = self.proposals.read().unwrap();
        let mut summaries = Vec::new();

        for (hash, proposal) in proposals_map.iter() {
            // Placeholder values for vote counts and status
            let yeas = 0; // TODO: Calculate from stored votes for this proposal
            let nays = 0;
            let abstains = 0;
            let is_established = proposal.n_time < chrono::Utc::now().timestamp() - (60*60*24); // Simplified
            let is_passing = yeas > nays; // Highly simplified
            let status = if is_passing { "passing" } else { "failing" }.to_string();
            
            // TODO: Convert CScript (proposal.address_script) to TWINS Address string
            let payee_address_placeholder = "TWINS_PAYEE_ADDRESS_PLACEHOLDER".to_string();
            // TODO: Calculate monthly amount based on total_amount and duration (block_end - block_start)
            let monthly_amount_placeholder = (proposal.n_amount as f64 / 100_000_000.0 / 1.0).to_string(); // Placeholder

            summaries.push(ApiBudgetProposalSummary {
                proposal_hash: hex::encode(hash),
                name: proposal.str_proposal_name.clone(),
                url: proposal.str_url.clone(),
                block_start: proposal.n_block_start,
                block_end: proposal.n_block_end,
                total_amount: (proposal.n_amount as f64 / 100_000_000.0).to_string(),
                monthly_amount: monthly_amount_placeholder,
                payee_address: payee_address_placeholder, 
                fee_tx_hash: hex::encode(proposal.n_fee_tx_hash),
                time_created: proposal.n_time,
                yeas,
                nays,
                abstains,
                is_established,
                is_passing,
                status,
            });
        }
        let total_items = summaries.len();
        // TODO: Implement actual pagination
        (summaries, total_items)
    }
} 