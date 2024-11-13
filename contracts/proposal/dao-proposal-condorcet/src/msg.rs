use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{CosmosMsg, Empty};

use dao_dao_macros::proposal_module_query;

use crate::config::UncheckedConfig;

pub type InstantiateMsg = UncheckedConfig;

#[cw_serde]
pub struct Choice {
    pub msgs: Vec<CosmosMsg<Empty>>,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    Propose {
        choices: Vec<Choice>,
    },
    Vote {
        proposal_id: u32,
        vote: Vec<u32>,
    },
    #[cw_orch(fn_name("proposal_execute"))]
    Execute {
        proposal_id: u32,
    },
    Close {
        proposal_id: u32,
    },
    SetConfig(UncheckedConfig),
}

#[proposal_module_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    #[returns(crate::proposal::ProposalResponse)]
    Proposal { id: u32 },
    #[returns(crate::config::Config)]
    Config {},
}
