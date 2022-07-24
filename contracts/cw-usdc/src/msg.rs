use cosmwasm_std::{Coin, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub subdenom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    ChangeTokenFactoryAdmin { new_admin: String },
    ChangeContractOwner { new_owner: String },
    SetMinter { address: String, allowance: Uint128 },
    SetBurner { address: String, allowance: Uint128 },
    SetBlacklister { address: String, status: bool },
    SetFreezer { address: String, status: bool },
    Mint { to_address: String, amount: Uint128 },
    Burn { amount: Uint128 },
    Blacklist { address: String, status: bool },
    Freeze { status: bool },
}

/// SudoMsg is only exposed for internal Cosmos SDK modules to call.
/// This is showing how we can expose "admin" functionality than can not be called by
/// external users or contracts, but only trusted (native/Go) code in the blockchain
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SudoMsg {
    BeforeSend {
        from: String,
        to: String,
        amount: Vec<Coin>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    // IsFrozen returns if the entire token transfer functionality is frozen
    IsFrozen {},
    // Denom returns the token denom that this contract is the admin for
    Denom {},
    // Owner returns the owner of the contract
    Owner {},
    // Allowance returns the allowance of the specified address
    BurnAllowance { address: String },
    // Allowances Enumerates over all allownances. Response: Vec<AllowanceResponse>
    BurnAllowances {start_after: Option<String>, limit: Option<u32>},
    // Allowance returns the allowance of the specified user
    MintAllowance { address: String },
    // Allowances Enumerates over all allownances. Response: Vec<AllowanceResponse>
    MintAllowances {start_after: Option<String>, limit: Option<u32>},
    // IsBlacklisted returns wether the user is blacklisted or not
    IsBlacklisted {address: String},
    // Blacklist Enumerates over all blacklisted addresses
    Blacklist { start_after: Option<String>, limit: Option<u32>},
    // IsBlacklister returns if the addres has blacklister privileges
    IsBlacklister { address: String},
    // Blacklisters Enumerates over all the addresses with blacklister privileges
    Blacklisters { start_after: Option<String>, limit: Option<u32>},
    // IsFreezer returns wether the address has freezerstatus 
    IsFreezer { address: String },
    // FreezerAllowances enumerates over all freezer addresses
    FreezerAllowances { start_after: Option<String>, limit: Option<u32> }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IsFrozenResponse {
    pub is_frozen: bool,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DenomResponse {
    pub denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OwnerResponse {
    pub address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AllowancesResponse {
    pub allowances: Vec<AllowanceResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AllowanceResponse {
    pub address: String,
    pub allowance: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StatusResponse {
    pub address: String,
    pub status: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BlacklistResponse {
    pub blacklist: Vec<StatusResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BlacklistersResponse {
    pub blacklisters : Vec<StatusResponse>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FreezerAllowancesResponse {
    pub freezers: Vec<StatusResponse>
}
