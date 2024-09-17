use cosmwasm_std::{Addr, StdResult, Storage};
use cw_storage_plus::{Item, Map};

use dao_voting::{approval::ApprovalProposal, proposal::SingleChoiceProposeMsg};

pub type Proposal = ApprovalProposal<SingleChoiceProposeMsg>;

pub const APPROVER: Item<Addr> = Item::new("approver");
pub const PENDING_PROPOSALS: Map<u64, Proposal> = Map::new("pending_proposals");
pub const COMPLETED_PROPOSALS: Map<u64, Proposal> = Map::new("completed_proposals");
pub const CREATED_PROPOSAL_TO_COMPLETED_PROPOSAL: Map<u64, u64> =
    Map::new("created_to_completed_proposal");

/// Used internally to track the current approval_id.
const CURRENT_ID: Item<u64> = Item::new("current_id");

pub(crate) fn advance_approval_id(store: &mut dyn Storage) -> StdResult<u64> {
    let id: u64 = CURRENT_ID.may_load(store)?.unwrap_or_default() + 1;
    CURRENT_ID.save(store, &id)?;
    Ok(id)
}
