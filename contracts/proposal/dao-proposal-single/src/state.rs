use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_hooks::Hooks;
use cw_storage_plus::{Item, Map};
use cw_utils::Duration;
use dao_voting::{
    pre_propose::ProposalCreationPolicy, threshold::Threshold, veto::VetoConfig, voting::Vote,
};

use crate::proposal::SingleChoiceProposal;

pub type Ballot = dao_voting::proposal::Ballot<Vote>;

/// The governance module's configuration.
#[cw_serde]
pub struct Config {
    /// The threshold a proposal must reach to complete.
    pub threshold: Threshold,
    /// The default maximum amount of time a proposal may be voted on
    /// before expiring.
    pub max_voting_period: Duration,
    /// The minimum amount of time a proposal must be open before
    /// passing. A proposal may fail before this amount of time has
    /// elapsed, but it will not pass. This can be useful for
    /// preventing governance attacks wherein an attacker aquires a
    /// large number of tokens and forces a proposal through.
    pub min_voting_period: Option<Duration>,
    /// If set to true only members may execute passed
    /// proposals. Otherwise, any address may execute a passed
    /// proposal.
    pub only_members_execute: bool,
    /// Allows changing votes before the proposal expires. If this is
    /// enabled proposals will not be able to complete early as final
    /// vote information is not known until the time of proposal
    /// expiration.
    pub allow_revoting: bool,
    /// The address of the DAO that this governance module is
    /// associated with.
    pub dao: Addr,
    /// If set to true proposals will be closed if their execution
    /// fails. Otherwise, proposals will remain open after execution
    /// failure. For example, with this enabled a proposal to send 5
    /// tokens out of a DAO's treasury with 4 tokens would be closed when
    /// it is executed. With this disabled, that same proposal would
    /// remain open until the DAO's treasury was large enough for it to be
    /// executed.
    pub close_proposal_on_execution_failure: bool,
    /// Optional veto configuration. If set to `None`, veto option
    /// is disabled. Otherwise contains the configuration for veto flow.
    pub veto: Option<VetoConfig>,
}

/// The current top level config for the module.  The "config" key was
/// previously used to store configs for v1 DAOs.
pub const CONFIG: Item<Config> = Item::new("config_v2");
/// The number of proposals that have been created.
pub const PROPOSAL_COUNT: Item<u64> = Item::new("proposal_count");
pub const PROPOSALS: Map<u64, SingleChoiceProposal> = Map::new("proposals_v2");
pub const BALLOTS: Map<(u64, &Addr), Ballot> = Map::new("ballots");
/// Consumers of proposal state change hooks.
pub const PROPOSAL_HOOKS: Hooks = Hooks::new("proposal_hooks");
/// Consumers of vote hooks.
pub const VOTE_HOOKS: Hooks = Hooks::new("vote_hooks");
/// The address of the pre-propose module associated with this
/// proposal module (if any).
pub const CREATION_POLICY: Item<ProposalCreationPolicy> = Item::new("creation_policy");
/// The address of the delegation module associated with this proposal module
/// (if any).
pub const DELEGATION_MODULE: Item<Addr> = Item::new("delegation_module");
