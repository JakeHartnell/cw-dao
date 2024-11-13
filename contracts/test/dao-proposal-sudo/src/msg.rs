use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::CosmosMsg;

#[cw_serde]
pub struct InstantiateMsg {
    pub root: String,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    #[cw_orch(fn_name("proposal_execute"))]
    Execute { msgs: Vec<CosmosMsg> },
}

#[cw_serde]
#[derive(cw_orch::QueryFns, QueryResponses)]
pub enum QueryMsg {
    #[returns(cosmwasm_std::Addr)]
    Admin {},
    #[returns(cosmwasm_std::Addr)]
    Dao {},
    #[returns(dao_interface::voting::InfoResponse)]
    Info {},
}
