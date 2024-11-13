use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::CustomMsg;

#[cw_serde]
pub struct MetadataExt {
    /// Optional on-chain role for this member, can be used by other contracts to enforce permissions
    pub role: Option<String>,
    /// The voting weight of this role
    pub weight: u64,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteExt {
    /// Add a new hook to be informed of all membership changes.
    /// Must be called by Admin
    AddHook { addr: String },
    /// Remove a hook. Must be called by Admin
    RemoveHook { addr: String },
    /// Update the token_uri for a particular NFT. Must be called by minter / admin
    UpdateTokenUri {
        token_id: String,
        token_uri: Option<String>,
    },
    /// Updates the voting weight of a token. Must be called by minter / admin
    UpdateTokenWeight { token_id: String, weight: u64 },
    /// Udates the role of a token. Must be called by minter / admin
    UpdateTokenRole {
        token_id: String,
        role: Option<String>,
    },
}
impl CustomMsg for ExecuteExt {}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryExt {
    /// Total weight at a given height
    #[returns(cw4::TotalWeightResponse)]
    TotalWeight { at_height: Option<u64> },
    /// Returns a list of Members
    #[returns(cw4::MemberListResponse)]
    ListMembers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns the weight of a certain member
    #[returns(cw4::MemberResponse)]
    Member {
        addr: String,
        at_height: Option<u64>,
    },
    /// Shows all registered hooks.
    #[returns(cw_controllers::HooksResponse)]
    Hooks {},
}
impl CustomMsg for QueryExt {}
