use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, StdResult, Storage, SubMsg, Uint128, WasmMsg};
use cw_hooks::Hooks;
use dao_voting::reply::mask_vote_hook_index;

/// An enum representing vote hooks, fired when new votes are cast.
#[cw_serde]
pub enum VoteHookMsg {
    NewVote {
        /// The proposal ID that was voted on.
        proposal_id: u64,
        /// The voter that cast the vote.
        voter: String,
        /// The vote that was cast.
        vote: String,
        /// The total voting power of the voter.
        power: Uint128,
        /// The individual voting power of the voter (excluding any delegated
        /// voting power).
        individual_power: Uint128,
        /// The block height at which the voting power is calculated.
        height: u64,
        /// Whether this is the first vote cast by this voter on this proposal.
        /// This will always be true if revoting is disabled.
        is_first_vote: bool,
    },
}

/// Prepares new vote hook messages. These messages reply on error
/// and have even reply IDs.
/// IDs are set to odd numbers to then be interleaved with the proposal hooks.
#[allow(clippy::too_many_arguments)]
pub fn new_vote_hooks(
    hooks: Hooks,
    storage: &dyn Storage,
    proposal_id: u64,
    voter: String,
    vote: String,
    power: Uint128,
    individual_power: Uint128,
    height: u64,
    is_first_vote: bool,
) -> StdResult<Vec<SubMsg>> {
    let msg = to_json_binary(&VoteHookExecuteMsg::VoteHook(VoteHookMsg::NewVote {
        proposal_id,
        voter,
        vote,
        power,
        individual_power,
        height,
        is_first_vote,
    }))?;
    let mut index: u64 = 0;
    hooks.prepare_hooks(storage, |a| {
        let execute = WasmMsg::Execute {
            contract_addr: a.to_string(),
            msg: msg.clone(),
            funds: vec![],
        };
        let masked_index = mask_vote_hook_index(index);
        let tmp = SubMsg::reply_on_error(execute, masked_index);
        index += 1;
        Ok(tmp)
    })
}

#[cw_serde]
pub enum VoteHookExecuteMsg {
    VoteHook(VoteHookMsg),
}
