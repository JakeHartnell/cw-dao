use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Uint128;

use crate::state::CheckedCounterparty;

/// Information about the token being used on one side of the escrow.
#[cw_serde]
pub enum TokenInfo {
    /// A native token.
    Native { denom: String, amount: Uint128 },
    /// A cw20 token.
    Cw20 {
        contract_addr: String,
        amount: Uint128,
    },
}

/// Information about a counterparty in this escrow transaction and
/// their promised funds.
#[cw_serde]
pub struct Counterparty {
    /// The address of the counterparty.
    pub address: String,
    /// The funds they have promised to provide.
    pub promise: TokenInfo,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub counterparty_one: Counterparty,
    pub counterparty_two: Counterparty,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    /// Used to provide cw20 tokens to satisfy a funds promise.
    Receive(cw20::Cw20ReceiveMsg),
    /// Provides native tokens to satisfy a funds promise.
    #[cw_orch(payable)]
    Fund {},
    /// Withdraws provided funds. Only allowed if the other
    /// counterparty has yet to provide their promised funds.
    Withdraw {},
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg {
    // Gets the current status of the escrow transaction.
    #[returns(crate::msg::StatusResponse)]
    Status {},
}

#[cw_serde]
pub struct StatusResponse {
    pub counterparty_one: CheckedCounterparty,
    pub counterparty_two: CheckedCounterparty,
}

#[cw_serde]
pub struct MigrateMsg {}
