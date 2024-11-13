use cosmwasm_schema::{cw_serde, schemars::JsonSchema, QueryResponses};
use cw_denom::UncheckedDenom;
use dao_interface::proposal::InfoResponse;
use dao_voting::{
    deposit::{CheckedDepositInfo, UncheckedDepositInfo},
    pre_propose::PreProposeSubmissionPolicy,
    status::Status,
};

#[cw_serde]
pub struct InstantiateMsg<InstantiateExt> {
    /// Information about the deposit requirements for this
    /// module. None if no deposit.
    pub deposit_info: Option<UncheckedDepositInfo>,
    /// The policy dictating who is allowed to submit proposals.
    pub submission_policy: PreProposeSubmissionPolicy,
    /// Extension for instantiation. The default implementation will
    /// do nothing with this data.
    pub extension: InstantiateExt,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg<ProposalMessage, ExecuteExt> {
    /// Creates a new proposal in the pre-propose module. MSG will be
    /// serialized and used as the proposal creation message.
    #[cw_orch(payable)]
    Propose { msg: ProposalMessage },

    /// Updates the configuration of this module. This will completely
    /// override the existing configuration. This new configuration
    /// will only apply to proposals created after the config is
    /// updated. Only the DAO may execute this message.
    UpdateConfig {
        /// If None, will remove the deposit. Backwards compatible.
        deposit_info: Option<UncheckedDepositInfo>,
        /// If None, will leave the submission policy in the config as-is.
        submission_policy: Option<PreProposeSubmissionPolicy>,
    },

    /// Perform more granular submission policy updates to allow for atomic
    /// operations that don't override others.
    UpdateSubmissionPolicy {
        /// Optionally add to the denylist. Works for any submission policy.
        denylist_add: Option<Vec<String>>,
        /// Optionally remove from denylist. Works for any submission policy.
        denylist_remove: Option<Vec<String>>,
        /// If using specific policy, optionally update the `dao_members` flag.
        set_dao_members: Option<bool>,
        /// If using specific policy, optionally add to the allowlist.
        allowlist_add: Option<Vec<String>>,
        /// If using specific policy, optionally remove from the allowlist.
        allowlist_remove: Option<Vec<String>>,
    },

    /// Withdraws funds inside of this contract to the message
    /// sender. The contracts entire balance for the specifed DENOM is
    /// withdrawn to the message sender. Only the DAO may call this
    /// method.
    ///
    /// This is intended only as an escape hatch in the event of a
    /// critical bug in this contract or it's proposal
    /// module. Withdrawing funds will cause future attempts to return
    /// proposal deposits to fail their transactions as the contract
    /// will have insufficent balance to return them. In the case of
    /// `cw-proposal-single` this transaction failure will cause the
    /// module to remove the pre-propose module from its proposal hook
    /// receivers.
    ///
    /// More likely than not, this should NEVER BE CALLED unless a bug
    /// in this contract or the proposal module it is associated with
    /// has caused it to stop receiving proposal hook messages, or if
    /// a critical security vulnerability has been found that allows
    /// an attacker to drain proposal deposits.
    Withdraw {
        /// The denom to withdraw funds for. If no denom is specified,
        /// the denomination currently configured for proposal
        /// deposits will be used.
        ///
        /// You may want to specify a denomination here if you are
        /// withdrawing funds that were previously accepted for
        /// proposal deposits but are not longer used due to an
        /// `UpdateConfig` message being executed on the contract.
        denom: Option<UncheckedDenom>,
    },

    /// Extension message. Contracts that extend this one should put
    /// their custom execute logic here. The default implementation
    /// will do nothing if this variant is executed.
    Extension { msg: ExecuteExt },

    /// Adds a proposal submitted hook. Fires when a new proposal is submitted
    /// to the pre-propose contract. Only the DAO may call this method.
    AddProposalSubmittedHook { address: String },

    /// Removes a proposal submitted hook. Only the DAO may call this method.
    RemoveProposalSubmittedHook { address: String },

    /// Handles proposal hook fired by the associated proposal
    /// module when a proposal is completed (ie executed or rejected).
    /// By default, the base contract will return deposits
    /// proposals, when they are closed, when proposals are executed, or,
    /// if it is refunding failed.
    ProposalCompletedHook {
        proposal_id: u64,
        new_status: Status,
    },
}

#[cw_serde]
#[derive(QueryResponses, cw_orch::QueryFns)]
pub enum QueryMsg<QueryExt>
where
    QueryExt: JsonSchema,
{
    /// Gets the proposal module that this pre propose module is
    /// associated with. Returns `Addr`.
    #[returns(cosmwasm_std::Addr)]
    ProposalModule {},
    /// Gets the DAO (dao-dao-core) module this contract is associated
    /// with. Returns `Addr`.
    #[returns(cosmwasm_std::Addr)]
    Dao {},
    /// Returns contract version info.
    #[returns(InfoResponse)]
    Info {},
    /// Gets the module's configuration.
    #[returns(crate::state::Config)]
    Config {},
    /// Gets the deposit info for the proposal identified by
    /// PROPOSAL_ID.
    #[returns(DepositInfoResponse)]
    DepositInfo { proposal_id: u64 },
    /// Returns whether or not the address can submit proposals.
    #[returns(bool)]
    CanPropose { address: String },
    /// Returns list of proposal submitted hooks.
    #[returns(cw_hooks::HooksResponse)]
    ProposalSubmittedHooks {},
    /// Extension for queries. The default implementation will do
    /// nothing if queried for will return `Binary::default()`.
    #[returns(cosmwasm_std::Binary)]
    QueryExtension { msg: QueryExt },
}

#[cw_serde]
pub struct DepositInfoResponse {
    /// The deposit that has been paid for the specified proposal.
    pub deposit_info: Option<CheckedDepositInfo>,
    /// The address that created the proposal.
    pub proposer: cosmwasm_std::Addr,
}

#[cw_serde]
pub enum MigrateMsg<MigrateExt>
where
    MigrateExt: JsonSchema,
{
    FromUnderV250 {
        /// Optionally set a new submission policy with more granular controls.
        /// If not set, the current policy will remain.
        policy: Option<PreProposeSubmissionPolicy>,
    },
    Extension {
        msg: MigrateExt,
    },
}
