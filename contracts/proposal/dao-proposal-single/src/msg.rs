use cosmwasm_schema::{cw_serde, QueryResponses};
use cw_utils::Duration;
use dao_dao_macros::proposal_module_query;
use dao_voting::{
    pre_propose::PreProposeInfo, proposal::SingleChoiceProposeMsg, threshold::Threshold,
    veto::VetoConfig, voting::Vote,
};

#[cw_serde]
pub struct InstantiateMsg {
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
    /// Information about what addresses may create proposals.
    pub pre_propose_info: PreProposeInfo,
    /// If set to true proposals will be closed if their execution
    /// fails. Otherwise, proposals will remain open after execution
    /// failure. For example, with this enabled a proposal to send 5
    /// tokens out of a DAO's treasury with 4 tokens would be closed when
    /// it is executed. With this disabled, that same proposal would
    /// remain open until the DAO's treasury was large enough for it to be
    /// executed.
    pub close_proposal_on_execution_failure: bool,
    /// Optional veto configuration for proposal execution.
    /// If set, proposals can only be executed after the timelock
    /// delay expiration.
    /// During this period an oversight account (`veto.vetoer`) can
    /// veto the proposal.
    pub veto: Option<VetoConfig>,
    /// The address of the delegation module to use for this proposal module (if
    /// any).
    pub delegation_module: Option<String>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Creates a proposal in the module.
    Propose(SingleChoiceProposeMsg),
    /// Votes on a proposal. Voting power is determined by the DAO's
    /// voting power module.
    Vote {
        /// The ID of the proposal to vote on.
        proposal_id: u64,
        /// The senders position on the proposal.
        vote: Vote,
        /// An optional rationale for why this vote was cast. This can
        /// be updated, set, or removed later by the address casting
        /// the vote.
        rationale: Option<String>,
    },
    /// Updates the sender's rationale for their vote on the specified
    /// proposal. Errors if no vote vote has been cast.
    UpdateRationale {
        proposal_id: u64,
        rationale: Option<String>,
    },
    /// Causes the messages associated with a passed proposal to be
    /// executed by the DAO.
    Execute {
        /// The ID of the proposal to execute.
        proposal_id: u64,
    },
    /// Callable only if veto is configured
    Veto {
        /// The ID of the proposal to veto.
        proposal_id: u64,
    },
    /// Closes a proposal that has failed (either not passed or timed
    /// out). If applicable this will cause the proposal deposit
    /// associated wth said proposal to be returned.
    Close {
        /// The ID of the proposal to close.
        proposal_id: u64,
    },
    /// Updates the governance module's config.
    UpdateConfig {
        /// The new proposal passing threshold. This will only apply
        /// to proposals created after the config update.
        threshold: Threshold,
        /// The default maximum amount of time a proposal may be voted
        /// on before expiring. This will only apply to proposals
        /// created after the config update.
        max_voting_period: Duration,
        /// The minimum amount of time a proposal must be open before
        /// passing. A proposal may fail before this amount of time has
        /// elapsed, but it will not pass. This can be useful for
        /// preventing governance attacks wherein an attacker aquires a
        /// large number of tokens and forces a proposal through.
        min_voting_period: Option<Duration>,
        /// If set to true only members may execute passed
        /// proposals. Otherwise, any address may execute a passed
        /// proposal. Applies to all outstanding and future proposals.
        only_members_execute: bool,
        /// Allows changing votes before the proposal expires. If this is
        /// enabled proposals will not be able to complete early as final
        /// vote information is not known until the time of proposal
        /// expiration.
        allow_revoting: bool,
        /// The address if tge DAO that this governance module is
        /// associated with.
        dao: String,
        /// If set to true proposals will be closed if their execution
        /// fails. Otherwise, proposals will remain open after execution
        /// failure. For example, with this enabled a proposal to send 5
        /// tokens out of a DAO's treasury with 4 tokens would be closed when
        /// it is executed. With this disabled, that same proposal would
        /// remain open until the DAO's treasury was large enough for it to be
        /// executed.
        close_proposal_on_execution_failure: bool,
        /// Optional time delay on proposal execution, during which the
        /// proposal may be vetoed.
        veto: Option<VetoConfig>,
    },
    /// Update's the proposal creation policy used for this
    /// module. Only the DAO may call this method.
    UpdatePreProposeInfo { info: PreProposeInfo },
    /// Update's the address of the delegation module associated with this
    /// proposal module. Only the DAO may call this method.
    UpdateDelegationModule { module: String },
    /// Adds an address as a consumer of proposal hooks. Consumers of
    /// proposal hooks have hook messages executed on them whenever
    /// the status of a proposal changes or a proposal is created. If
    /// a consumer contract errors when handling a hook message it
    /// will be removed from the list of consumers.
    AddProposalHook { address: String },
    /// Removes a consumer of proposal hooks.
    RemoveProposalHook { address: String },
    /// Adds an address as a consumer of vote hooks. Consumers of vote
    /// hooks have hook messages executed on them whenever the a vote
    /// is cast. If a consumer contract errors when handling a hook
    /// message it will be removed from the list of consumers.
    AddVoteHook { address: String },
    /// Removed a consumer of vote hooks.
    RemoveVoteHook { address: String },
}

#[proposal_module_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Gets the proposal module's config.
    #[returns(crate::state::Config)]
    Config {},
    /// Gets information about a proposal.
    #[returns(crate::query::ProposalResponse)]
    Proposal { proposal_id: u64 },
    /// Lists all the proposals that have been cast in this
    /// module.
    #[returns(crate::query::ProposalListResponse)]
    ListProposals {
        /// The proposal ID to start listing proposals after. For
        /// example, if this is set to 2 proposals with IDs 3 and
        /// higher will be returned.
        start_after: Option<u64>,
        /// The maximum number of proposals to return as part of this
        /// query. If no limit is set a max of 30 proposals will be
        /// returned.
        limit: Option<u64>,
    },
    /// Lists all of the proposals that have been cast in this module
    /// in decending order of proposal ID.
    #[returns(crate::query::ProposalListResponse)]
    ReverseProposals {
        /// The proposal ID to start listing proposals before. For
        /// example, if this is set to 6 proposals with IDs 5 and
        /// lower will be returned.
        start_before: Option<u64>,
        /// The maximum number of proposals to return as part of this
        /// query. If no limit is set a max of 30 proposals will be
        /// returned.
        limit: Option<u64>,
    },
    /// Returns a voters position on a propsal.
    #[returns(crate::query::VoteResponse)]
    GetVote { proposal_id: u64, voter: String },
    /// Lists all of the votes that have been cast on a
    /// proposal.
    #[returns(crate::query::VoteListResponse)]
    ListVotes {
        /// The proposal to list the votes of.
        proposal_id: u64,
        /// The voter to start listing votes after. Ordering is done
        /// alphabetically.
        start_after: Option<String>,
        /// The maximum number of votes to return in response to this
        /// query. If no limit is specified a max of 30 are returned.
        limit: Option<u64>,
    },
    /// Returns the number of proposals that have been created in this module.
    #[returns(::std::primitive::u64)]
    ProposalCount {},
    /// Gets the current proposal creation policy for this module.
    #[returns(::dao_voting::pre_propose::ProposalCreationPolicy)]
    ProposalCreationPolicy {},
    /// Gets the address of the delegation module associated with this
    /// proposal module (if any).
    #[returns(Option<::cosmwasm_std::Addr>)]
    DelegationModule {},
    /// Lists all of the consumers of proposal hooks for this module.
    #[returns(::cw_hooks::HooksResponse)]
    ProposalHooks {},
    /// Lists all of the consumers of vote hooks for this module.
    #[returns(::cw_hooks::HooksResponse)]
    VoteHooks {},
}

#[cw_serde]
pub enum MigrateMsg {
    FromV1 {
        /// This field was not present in DAO DAO v1. To migrate, a
        /// value must be specified.
        ///
        /// If set to true proposals will be closed if their execution
        /// fails. Otherwise, proposals will remain open after execution
        /// failure. For example, with this enabled a proposal to send 5
        /// tokens out of a DAO's treasury with 4 tokens would be closed when
        /// it is executed. With this disabled, that same proposal would
        /// remain open until the DAO's treasury was large enough for it to be
        /// executed.
        close_proposal_on_execution_failure: bool,
        /// This field was not present in DAO DAO v1. To migrate, a
        /// value must be specified.
        ///
        /// This contains information about how a pre-propose module may be configured.
        /// If set to "AnyoneMayPropose", there will be no pre-propose module and consequently,
        /// no deposit or membership checks when submitting a proposal. The "ModuleMayPropose"
        /// option allows for instantiating a prepropose module which will handle deposit verification and return logic.
        pre_propose_info: PreProposeInfo,
        /// This field was not present in DAO DAO v1. To migrate, a
        /// value must be specified.
        ///
        /// optional configuration for veto feature
        veto: Option<VetoConfig>,
    },
    FromCompatible {},
}
