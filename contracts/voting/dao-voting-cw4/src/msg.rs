use cosmwasm_schema::{cw_serde, QueryResponses};
use dao_dao_macros::voting_module_query;

#[cw_serde]
pub enum GroupContract {
    Existing {
        address: String,
    },
    New {
        cw4_group_code_id: u64,
        initial_members: Vec<cw4::Member>,
    },
}

#[cw_serde]
pub struct InstantiateMsg {
    pub group_contract: GroupContract,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {}

#[voting_module_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(cosmwasm_std::Addr)]
    GroupContract {},
}

#[cw_serde]
pub struct MigrateMsg {}
