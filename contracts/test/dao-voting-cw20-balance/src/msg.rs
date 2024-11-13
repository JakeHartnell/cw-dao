use cosmwasm_schema::{cw_serde, QueryResponses};

use cw20::Cw20Coin;
use cw20_base::msg::InstantiateMarketingInfo;

use dao_dao_macros::{cw20_token_query, voting_module_query};

#[cw_serde]
pub enum TokenInfo {
    Existing {
        address: String,
    },
    New {
        code_id: u64,
        label: String,

        name: String,
        symbol: String,
        decimals: u8,
        initial_balances: Vec<Cw20Coin>,
        marketing: Option<InstantiateMarketingInfo>,
    },
}

#[cw_serde]
pub struct InstantiateMsg {
    pub token_info: TokenInfo,
}

#[cw_serde]
pub enum ExecuteMsg {}

#[cw20_token_query]
#[voting_module_query]
#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {}
