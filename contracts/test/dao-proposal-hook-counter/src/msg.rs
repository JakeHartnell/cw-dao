use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;
use dao_hooks::{proposal::ProposalHookMsg, stake::StakeChangedHookMsg, vote::VoteHookMsg};

#[cw_serde]
pub struct InstantiateMsg {
    pub should_error: bool, // Debug flag to test when hooks fail over
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    ProposalHook(ProposalHookMsg),
    StakeChangeHook(StakeChangedHookMsg),
    VoteHook(VoteHookMsg),
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(Uint128)]
    StakeCounter {},
    #[returns(u64)]
    VoteCounter {},
    #[returns(u64)]
    ProposalCounter {},
    #[returns(u64)]
    StatusChangedCounter {},
}

#[cw_serde]
pub struct CountResponse {
    pub count: u64,
}
