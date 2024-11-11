use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{CosmosMsg, Decimal, Uint128};
use cw4::MemberChangedHookMsg;
use cw_ownable::{cw_ownable_execute, cw_ownable_query};
use dao_hooks::{nft_stake::NftStakeChangedHookMsg, stake::StakeChangedHookMsg};

use crate::state::{Reset, Vote};

type GaugeId = u64;

#[cw_serde]
pub struct InstantiateMsg {
    /// Address of contract to that contains all voting powers (where we query)
    pub voting_powers: String,
    /// Address that will call voting power change hooks (often same as voting power contract)
    pub hook_caller: String,
    /// Optional Address that can add new gauges or stop them
    pub owner: String,
    /// Allow attaching multiple adaptors during instantiation.
    /// Important, as instantiation and CreateGauge both come from DAO proposals
    /// and without this argument, you need 2 cycles to create and configure a gauge
    pub gauges: Option<Vec<GaugeConfig>>,
}

#[cw_serde]
pub struct GaugeConfig {
    /// Name of the gauge (for UI)
    pub title: String,
    /// Address of contract to serve gauge-specific info (AdapterQueryMsg)
    pub adapter: String,
    /// Frequency (in seconds) the gauge executes messages, typically something like 7*86400
    pub epoch_size: u64,
    /// Minimum percentage of votes needed by a given option to be in the selected set.
    /// If unset, there is no minimum percentage, just the `max_options_selected` limit.
    pub min_percent_selected: Option<Decimal>,
    /// Maximum number of Options to make the selected set. Needed even with
    /// `min_percent_selected` to provide some guarantees on gas usage of this query.
    pub max_options_selected: u32,
    // Any votes above that percentage will be discarded
    pub max_available_percentage: Option<Decimal>,
    /// If set, the gauge can be reset periodically, every `reset_epoch` seconds.
    pub reset_epoch: Option<u64>,
    /// If set, the gauge will disable itself after this many epochs. This count will not be reset if `reset_epoch` is set.
    pub total_epochs: Option<u64>,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    /// Updates gauge voting power in Token DAOs when a user stakes or unstakes
    StakeChangeHook(StakeChangedHookMsg),
    /// Updates gauge voting power in NFT DAOs when a user stakes or unstakes
    NftStakeChangeHook(NftStakeChangedHookMsg),
    /// Updates gauge voting power for membership changes
    MemberChangedHook(MemberChangedHookMsg),
    /// This creates a new Gauge, returns CreateGaugeReply JSON-encoded in the data field.
    /// Can only be called by owner
    CreateGauge(GaugeConfig),
    /// Allows owner to update certain parameters of GaugeConfig.
    /// If you want to change next_epoch value, you need to use migration.
    UpdateGauge {
        gauge_id: u64,
        epoch_size: Option<u64>,
        // Some<0> would set min_percent_selected to None
        min_percent_selected: Option<Decimal>,
        max_options_selected: Option<u32>,
        max_available_percentage: Option<Decimal>,
        epoch_limit: Option<u64>,
    },
    /// Stops a given gauge, meaning it will not execute any more messages,
    /// Or receive any more updates on MemberChangedHook.
    /// Ideally, this will allow for eventual deletion of all data on that gauge
    StopGauge { gauge: u64 },
    /// Resets all votes on a given gauge if it is configured to be periodically reset and the epoch has passed.
    /// One call to this will only clear `batch_size` votes to prevent gas exhaustion. Call repeatedly to clear all votes.
    ResetGauge { gauge: u64, batch_size: u32 },
    // WISH: make this implicit - call it inside PlaceVote.
    // If not, I would just make it invisible to user in UI (smart client adds it if needed)
    /// Try to add an option. Error if no such gauge, or option already registered.
    /// Otherwise check adapter and error if invalid.
    /// Can be called by anyone, not just owner
    AddOption { gauge: u64, option: String },
    /// Allows the owner to remove an option. This is useful if the option is no longer valid
    /// or if the owner wants to remove all votes from a valid option.
    RemoveOption { gauge: u64, option: String },
    /// Place your vote on the gauge. Can be updated anytime
    PlaceVotes {
        /// Gauge to vote on
        gauge: u64,
        /// The options to put my vote on, along with proper weights (must sum up to 1.0)
        /// "None" means remove existing votes and abstain
        votes: Option<Vec<Vote>>,
    },
    /// Takes a sample of the current tally and execute the proper messages to make it work
    Execute { gauge: u64 },
}

#[cw_serde]
pub struct CreateGaugeReply {
    /// Id of the gauge that was just created
    pub id: u64,
    pub addr: String,
}

/// Queries the gauge exposes
#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// General contract info
    #[returns(dao_interface::voting::InfoResponse)]
    Info {},
    /// Returns details for a specific gauge.
    #[returns(GaugeResponse)]
    Gauge { id: u64 },
    /// List all gauges
    #[returns(ListGaugesResponse)]
    ListGauges {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    /// Returns the vote for a given voter
    #[returns(VoteResponse)]
    Vote { gauge: u64, voter: String },
    /// Returns a list of all unexpired votes for a specific gauge-id
    #[returns(ListVotesResponse)]
    ListVotes {
        gauge: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns a list of all options available to vote for a specific gauge-id
    #[returns(ListOptionsResponse)]
    ListOptions {
        gauge: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns the selected messages that were determined by voting
    #[returns(SelectedSetResponse)]
    SelectedSet { gauge: u64 },
    /// Returns the last selected messages that were executed by the DAO
    #[returns(LastExecutedSetResponse)]
    LastExecutedSet { gauge: u64 },
}

/// Information about one gauge
#[cw_serde]
pub struct GaugeResponse {
    pub id: u64,
    /// Name of the gauge (for UI)
    pub title: String,
    /// Address of contract to serve gauge-specific info (AdapterQueryMsg)
    pub adapter: String,
    /// Frequency (in seconds) the gauge executes messages, typically something like 7*86400
    pub epoch_size: u64,
    /// Total epoch duration
    pub total_epochs: Option<u64>,
    /// Minimum percentage of votes needed by a given option to be in the selected set.
    /// If unset, there is no minimum percentage, just the `max_options_selected` limit.
    pub min_percent_selected: Option<Decimal>,
    /// Maximum number of Options to make the selected set. Needed even with
    /// `min_percent_selected` to provide some guarantees on gas usage of this query.
    pub max_options_selected: u32,
    // Any votes above that percentage will be discarded
    pub max_available_percentage: Option<Decimal>,
    /// True if the gauge is stopped
    pub is_stopped: bool,
    /// UNIX time (seconds) when next epoch may be executed. May be future or past
    pub next_epoch: u64,
    /// Set this in migration if the gauge should be periodically reset
    pub reset: Option<Reset>,
}

/// Information about one gauge
#[cw_serde]
pub struct ListGaugesResponse {
    pub gauges: Vec<GaugeResponse>,
}

/// Information about a vote that was cast.
#[cw_serde]
pub struct VoteInfo {
    /// The address that voted.
    pub voter: String,
    /// List of all votes with power
    pub votes: Vec<Vote>,
    /// Timestamp when vote was cast.
    /// Allow `None` for 0-cost migration from current data
    pub cast: Option<u64>,
}

/// Information about a vote.
#[cw_serde]
pub struct VoteResponse {
    /// None if no such vote, Some otherwise.
    pub vote: Option<VoteInfo>,
}

/// Information about all votes on the gauge
#[cw_serde]
pub struct ListVotesResponse {
    pub votes: Vec<VoteInfo>,
}

/// List all available options ordered by the option string.
/// Also returns the current voting power assigned to that option.
/// You will need to paginate to collect them all.
#[cw_serde]
pub struct ListOptionsResponse {
    pub options: Vec<(String, Uint128)>,
}

/// List the options that were selected in the last executed set.
#[cw_serde]
pub struct LastExecutedSetResponse {
    /// `None` if no vote has been executed yet
    pub votes: Option<Vec<(String, Uint128)>>,
}

/// List the top options by power that would make it into the selected set.
/// Ordered from highest votes to lowest
#[cw_serde]
pub struct SelectedSetResponse {
    pub votes: Vec<(String, Uint128)>,
}

/// Queries the gauge requires from the adapter contract in order to function
#[cw_serde]
#[derive(QueryResponses)]
pub enum AdapterQueryMsg {
    #[returns(AllOptionsResponse)]
    AllOptions {},
    #[returns(CheckOptionResponse)]
    CheckOption { option: String },
    #[returns(SampleGaugeMsgsResponse)]
    SampleGaugeMsgs {
        /// option along with weight
        /// sum of all weights should be 1.0 (within rounding error)
        selected: Vec<(String, Decimal)>,
    },
}

#[cw_serde]
pub struct AllOptionsResponse {
    pub options: Vec<String>,
}

#[cw_serde]
pub struct CheckOptionResponse {
    pub valid: bool,
}

#[cw_serde]
pub struct SampleGaugeMsgsResponse {
    // NOTE: I think we will never need CustomMsg here, any reason we should include??
    pub execute: Vec<CosmosMsg>,
}

#[cw_serde]
pub struct MigrateMsg {
    pub gauge_config: Option<Vec<(GaugeId, GaugeMigrationConfig)>>,
}

#[cw_serde]
#[derive(Default)]
pub struct GaugeMigrationConfig {
    /// When the next epoch should be executed
    pub next_epoch: Option<u64>,
    /// If set, the gauge will be reset periodically
    pub reset: Option<ResetMigrationConfig>,
}

#[cw_serde]
pub struct ResetMigrationConfig {
    /// How often to reset the gauge (in seconds)
    pub reset_epoch: u64,
    /// When to start the first reset
    pub next_reset: u64,
}
