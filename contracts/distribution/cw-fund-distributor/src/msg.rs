use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_utils::Duration;

#[cw_serde]
pub struct InstantiateMsg {
    // To determine voting power
    pub voting_contract: String,
    // period after which the funds can be claimed
    pub funding_period: Duration,
    // snapshot for evaluating the voting power
    pub distribution_height: u64,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    Receive(cw20::Cw20ReceiveMsg),
    #[cw_orch(payable)]
    FundNative {},
    ClaimCW20 {
        tokens: Vec<String>,
    },
    ClaimNatives {
        denoms: Vec<String>,
    },
    ClaimAll {},
}

#[cw_serde]
pub enum QueryMsg {
    TotalPower {},
    VotingContract {},
    NativeDenoms {},
    CW20Tokens {},
    NativeEntitlement {
        sender: Addr,
        denom: String,
    },
    CW20Entitlement {
        sender: Addr,
        token: String,
    },
    NativeEntitlements {
        sender: Addr,
        start_at: Option<String>,
        limit: Option<u32>,
    },
    CW20Entitlements {
        sender: Addr,
        start_at: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct VotingContractResponse {
    // voting power contract being used
    pub contract: Addr,
    // height at which voting power is being determined
    pub distribution_height: u64,
}

#[cw_serde]
pub struct TotalPowerResponse {
    // total power at the distribution height
    pub total_power: Uint128,
}

#[cw_serde]
pub enum MigrateMsg {
    RedistributeUnclaimedFunds { distribution_height: u64 },
}

#[cw_serde]
pub struct DenomResponse {
    pub contract_balance: Uint128,
    pub denom: String,
}

#[cw_serde]
pub struct CW20Response {
    pub contract_balance: Uint128,
    pub token: String,
}

#[cw_serde]
pub struct NativeEntitlementResponse {
    pub amount: Uint128,
    pub denom: String,
}

#[cw_serde]
pub struct CW20EntitlementResponse {
    pub amount: Uint128,
    pub token_contract: Addr,
}
