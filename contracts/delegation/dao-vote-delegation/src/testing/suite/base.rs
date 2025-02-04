use std::ops::{Deref, DerefMut};

use cosmwasm_std::{Addr, Decimal, Uint128};
use dao_interface::helpers::{OptionalUpdate, Update};
use dao_testing::DaoTestingSuiteBase;

use crate::ContractError;

use super::super::tests::dao_vote_delegation_contract;

pub struct DaoVoteDelegationTestingSuiteBase {
    /// base testing suite that we're extending
    pub base: DaoTestingSuiteBase,

    // initial config
    pub vp_cap_percent: Option<Decimal>,
    pub delegation_validity_blocks: Option<u64>,
    pub max_delegations: Option<u64>,

    /// DAO core address
    pub dao_core_addr: Addr,

    /// delegation code ID
    pub delegation_code_id: u64,
    /// delegation contract address
    pub delegation_addr: Addr,
}

// allow direct access to base testing suite methods
impl Deref for DaoVoteDelegationTestingSuiteBase {
    type Target = DaoTestingSuiteBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

// allow direct access to base testing suite methods
impl DerefMut for DaoVoteDelegationTestingSuiteBase {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

// CONSTRUCTOR
impl DaoVoteDelegationTestingSuiteBase {
    pub fn new() -> Self {
        let mut base = DaoTestingSuiteBase::base();
        let mut suite = base.cw4();

        let delegation_code_id = suite.store(dao_vote_delegation_contract);

        Self {
            base,

            vp_cap_percent: None,
            delegation_validity_blocks: None,
            max_delegations: None,

            dao_core_addr: Addr::unchecked(""),

            delegation_code_id,
            delegation_addr: Addr::unchecked(""),
        }
    }
}

// EXECUTIONS
impl DaoVoteDelegationTestingSuiteBase {
    /// register a user as a delegate
    pub fn register(&mut self, delegate: impl Into<String>) {
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            delegate,
            delegation_addr,
            &crate::msg::ExecuteMsg::Register {},
            &[],
        );
    }

    /// unregister a delegate
    pub fn unregister(&mut self, delegate: impl Into<String>) {
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            delegate,
            delegation_addr,
            &crate::msg::ExecuteMsg::Unregister {},
            &[],
        );
    }

    /// create or update a delegation
    pub fn delegate(
        &mut self,
        delegator: impl Into<String>,
        delegate: impl Into<String>,
        percent: Decimal,
    ) {
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            delegator,
            delegation_addr,
            &crate::msg::ExecuteMsg::Delegate {
                delegate: delegate.into(),
                percent,
            },
            &[],
        );
    }

    /// create or update a delegation and expect an error
    pub fn delegate_error(
        &mut self,
        delegator: impl Into<String>,
        delegate: impl Into<String>,
        percent: Decimal,
    ) -> ContractError {
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_err(
            delegator,
            delegation_addr,
            &crate::msg::ExecuteMsg::Delegate {
                delegate: delegate.into(),
                percent,
            },
            &[],
        )
    }

    /// revoke a delegation
    pub fn undelegate(&mut self, delegator: impl Into<String>, delegate: impl Into<String>) {
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            delegator,
            delegation_addr,
            &crate::msg::ExecuteMsg::Undelegate {
                delegate: delegate.into(),
            },
            &[],
        );
    }

    /// update voting power hook callers
    pub fn update_voting_power_hook_callers(
        &mut self,
        add: Option<Vec<String>>,
        remove: Option<Vec<String>>,
    ) {
        let core_addr = self.dao_core_addr.clone();
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            core_addr,
            delegation_addr,
            &crate::msg::ExecuteMsg::UpdateVotingPowerHookCallers { add, remove },
            &[],
        );
    }

    /// sync proposal modules
    pub fn sync_proposal_modules(&mut self, start_after: Option<String>, limit: Option<u32>) {
        let core_addr = self.dao_core_addr.clone();
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            core_addr,
            delegation_addr,
            &crate::msg::ExecuteMsg::SyncProposalModules { start_after, limit },
            &[],
        );
    }

    /// update VP cap percent
    pub fn update_vp_cap_percent(&mut self, vp_cap_percent: Option<Decimal>) {
        let core_addr = self.dao_core_addr.clone();
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            core_addr,
            delegation_addr,
            &crate::msg::ExecuteMsg::UpdateConfig {
                vp_cap_percent: OptionalUpdate(Some(
                    vp_cap_percent.map_or(Update::Clear, Update::Set),
                )),
                delegation_validity_blocks: OptionalUpdate(None),
                max_delegations: None,
            },
            &[],
        );
    }

    /// update delegation validity blocks
    pub fn update_delegation_validity_blocks(&mut self, delegation_validity_blocks: Option<u64>) {
        let core_addr = self.dao_core_addr.clone();
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            core_addr,
            delegation_addr,
            &crate::msg::ExecuteMsg::UpdateConfig {
                vp_cap_percent: OptionalUpdate(None),
                delegation_validity_blocks: OptionalUpdate(Some(
                    delegation_validity_blocks.map_or(Update::Clear, Update::Set),
                )),
                max_delegations: None,
            },
            &[],
        );
    }

    /// update max delegations
    pub fn update_max_delegations(&mut self, max_delegations: u64) {
        let core_addr = self.dao_core_addr.clone();
        let delegation_addr = self.delegation_addr.clone();
        self.execute_smart_ok(
            core_addr,
            delegation_addr,
            &crate::msg::ExecuteMsg::UpdateConfig {
                vp_cap_percent: OptionalUpdate(None),
                delegation_validity_blocks: OptionalUpdate(None),
                max_delegations: Some(max_delegations),
            },
            &[],
        );
    }
}

/// QUERIES
impl DaoVoteDelegationTestingSuiteBase {
    /// get whether a delegate is registered
    pub fn registered(&self, delegate: impl Into<String>, height: Option<u64>) -> bool {
        self.querier()
            .query_wasm_smart::<dao_voting::delegation::RegistrationResponse>(
                &self.delegation_addr,
                &crate::msg::QueryMsg::Registration {
                    delegate: delegate.into(),
                    height,
                },
            )
            .unwrap()
            .registered
    }

    /// get the delegates
    pub fn delegates(
        &self,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> Vec<dao_voting::delegation::DelegateResponse> {
        self.querier()
            .query_wasm_smart::<dao_voting::delegation::DelegatesResponse>(
                &self.delegation_addr,
                &crate::msg::QueryMsg::Delegates { start_after, limit },
            )
            .unwrap()
            .delegates
    }

    /// get the delegations
    pub fn delegations(
        &self,
        delegator: impl Into<String>,
        height: Option<u64>,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> dao_voting::delegation::DelegationsResponse {
        self.querier()
            .query_wasm_smart(
                &self.delegation_addr,
                &crate::msg::QueryMsg::Delegations {
                    delegator: delegator.into(),
                    height,
                    offset,
                    limit,
                },
            )
            .unwrap()
    }

    /// get the unvoted delegated voting power for a proposal
    pub fn unvoted_delegated_voting_power(
        &self,
        delegate: impl Into<String>,
        proposal_module: impl Into<String>,
        proposal_id: u64,
        start_height: u64,
    ) -> dao_voting::delegation::UnvotedDelegatedVotingPowerResponse {
        self.querier()
            .query_wasm_smart(
                &self.delegation_addr,
                &crate::msg::QueryMsg::UnvotedDelegatedVotingPower {
                    delegate: delegate.into(),
                    proposal_module: proposal_module.into(),
                    proposal_id,
                    height: start_height,
                },
            )
            .unwrap()
    }

    /// get the proposal modules
    pub fn proposal_modules(&self, start_after: Option<String>, limit: Option<u32>) -> Vec<Addr> {
        self.querier()
            .query_wasm_smart(
                &self.delegation_addr,
                &crate::msg::QueryMsg::ProposalModules { start_after, limit },
            )
            .unwrap()
    }

    /// get the voting power hook callers
    pub fn voting_power_hook_callers(
        &self,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> Vec<Addr> {
        self.querier()
            .query_wasm_smart(
                &self.delegation_addr,
                &crate::msg::QueryMsg::VotingPowerHookCallers { start_after, limit },
            )
            .unwrap()
    }

    /// get the config
    pub fn config(&self) -> dao_voting::delegation::Config {
        self.querier()
            .query_wasm_smart(&self.delegation_addr, &crate::msg::QueryMsg::Config {})
            .unwrap()
    }
}

/// ASSERTIONS
impl DaoVoteDelegationTestingSuiteBase {
    /// assert that there are N delegations
    pub fn assert_delegations_count(&self, delegator: impl Into<String>, count: u32) {
        let delegations = self.delegations(delegator, None, None, None);
        assert_eq!(delegations.delegations.len() as u32, count);
    }

    /// assert that there are N active delegations
    pub fn assert_active_delegations_count(&self, delegator: impl Into<String>, count: u32) {
        let delegations = self.delegations(delegator, None, None, None);
        assert_eq!(
            delegations.delegations.iter().filter(|d| d.active).count() as u32,
            count
        );
    }

    /// assert that an active delegation exists
    pub fn assert_delegation(
        &self,
        delegator: impl Into<String>,
        delegate: impl Into<String> + Copy,
        percent: Decimal,
    ) {
        let delegations = self.delegations(delegator, None, None, None);
        assert!(delegations
            .delegations
            .iter()
            .any(|d| d.delegate == delegate.into() && d.percent == percent && d.active));
    }

    /// assert that a delegate is registered
    pub fn assert_delegate_registered(
        &self,
        delegate: impl Into<String> + Copy,
        height: Option<u64>,
    ) {
        let registered = self.registered(delegate, height);
        assert!(registered);
    }

    /// assert that a delegate is not registered
    pub fn assert_delegate_not_registered(
        &self,
        delegate: impl Into<String> + Copy,
        height: Option<u64>,
    ) {
        let registered = self.registered(delegate, height);
        assert!(!registered);
    }

    /// assert that there are N delegates
    pub fn assert_delegates_count(&self, count: u32) {
        let delegates = self.delegates(None, Some(count));
        assert_eq!(delegates.len() as u32, count);
    }

    /// assert a delegate's total delegated voting power
    pub fn assert_delegate_total_delegated_vp(
        &self,
        delegate: impl Into<String> + Copy,
        expected_total: impl Into<Uint128>,
    ) {
        let delegate_total = self
            .delegates(None, None)
            .into_iter()
            .find(|d| d.delegate == delegate.into())
            .unwrap()
            .power;
        assert_eq!(delegate_total, expected_total.into());
    }

    /// assert a delegate's total UDVP on a proposal
    pub fn assert_total_udvp(
        &self,
        delegate: impl Into<String>,
        proposal_module: impl Into<String>,
        proposal_id: u64,
        start_height: u64,
        total: impl Into<Uint128>,
    ) {
        let udvp = self.unvoted_delegated_voting_power(
            delegate,
            proposal_module,
            proposal_id,
            start_height,
        );
        assert_eq!(udvp.total, total.into());
    }

    /// assert a delegate's effective UDVP on a proposal
    pub fn assert_effective_udvp(
        &self,
        delegate: impl Into<String>,
        proposal_module: impl Into<String>,
        proposal_id: u64,
        start_height: u64,
        effective: impl Into<Uint128>,
    ) {
        let udvp = self.unvoted_delegated_voting_power(
            delegate,
            proposal_module,
            proposal_id,
            start_height,
        );
        assert_eq!(udvp.effective, effective.into());
    }

    /// assert that the max delegations is set
    pub fn assert_max_delegations(&self, expected: u64) {
        let config = self.config();
        assert_eq!(config.max_delegations, expected);
    }
}
