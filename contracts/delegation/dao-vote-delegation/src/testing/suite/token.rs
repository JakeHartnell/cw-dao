use std::ops::{Deref, DerefMut};

use cosmwasm_std::{coins, Decimal, Uint128};
use cw_multi_test::{BankSudo, SudoMsg};
use dao_interface::token::InitialBalance;
use dao_testing::{DaoTestingSuite, TokenTestDao};

use super::base::DaoVoteDelegationTestingSuiteBase;

pub struct TokenDaoVoteDelegationTestingSuite {
    /// base testing suite that we're extending
    pub base: DaoVoteDelegationTestingSuiteBase,

    /// token-based voting DAO
    pub dao: TokenTestDao,
    /// members of the DAO
    pub members: Vec<InitialBalance>,
}

// allow direct access to base testing suite methods
impl Deref for TokenDaoVoteDelegationTestingSuite {
    type Target = DaoVoteDelegationTestingSuiteBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

// allow direct access to base testing suite methods
impl DerefMut for TokenDaoVoteDelegationTestingSuite {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

// CONSTRUCTOR
impl TokenDaoVoteDelegationTestingSuite {
    pub fn new() -> Self {
        let mut base = DaoVoteDelegationTestingSuiteBase::new();
        let mut suite = base.token();

        let members = suite.initial_balances.clone();
        let dao = suite.dao();

        base.dao_core_addr = dao.core_addr.clone();

        Self { base, dao, members }
    }

    pub fn with_vp_cap_percent(mut self, vp_cap_percent: Decimal) -> Self {
        self.vp_cap_percent = Some(vp_cap_percent);
        self
    }

    pub fn with_delegation_validity_blocks(mut self, delegation_validity_blocks: u64) -> Self {
        self.delegation_validity_blocks = Some(delegation_validity_blocks);
        self
    }

    pub fn with_max_delegations(mut self, max_delegations: u64) -> Self {
        self.max_delegations = Some(max_delegations);
        self
    }

    pub fn build(mut self) -> Self {
        let code_id = self.delegation_code_id;
        let core_addr = self.dao.core_addr.clone();
        let voting_module_addr = self.dao.voting_module_addr.clone();
        let vp_cap_percent = self.vp_cap_percent;
        let delegation_validity_blocks = self.delegation_validity_blocks;
        let max_delegations = self.max_delegations;

        self.delegation_addr = self.instantiate(
            code_id,
            &core_addr,
            &crate::msg::InstantiateMsg {
                dao: None,
                vp_hook_callers: Some(vec![voting_module_addr.to_string()]),
                no_sync_proposal_modules: None,
                vp_cap_percent,
                delegation_validity_blocks,
                max_delegations,
            },
            &[],
            "delegation",
            Some(core_addr.to_string()),
        );

        self.setup_delegation_module();

        self
    }

    /// set up delegation module by adding necessary hooks and adding it to the
    /// proposal modules
    pub fn setup_delegation_module(&mut self) {
        let dao = self.dao.clone();
        let delegation_addr = self.delegation_addr.to_string();

        // add voting power changed hook to voting module
        self.execute_smart_ok(
            &dao.core_addr,
            &dao.voting_module_addr,
            &dao_voting_token_staked::msg::ExecuteMsg::AddHook {
                addr: delegation_addr.clone(),
            },
            &[],
        );

        // add vote hook to all proposal modules
        self.add_vote_hook(&dao, &delegation_addr);

        // set the delegation module for all proposal modules
        self.set_delegation_module(&dao, &delegation_addr);
    }

    /// mint tokens
    pub fn mint(&mut self, recipient: impl Into<String>, amount: impl Into<u128>) {
        let denom = self.dao.x.denom.clone();
        self.app
            .sudo(SudoMsg::Bank({
                BankSudo::Mint {
                    to_address: recipient.into(),
                    amount: coins(amount.into(), denom),
                }
            }))
            .unwrap();
    }

    /// stake tokens
    pub fn stake(&mut self, staker: impl Into<String>, amount: impl Into<u128>) {
        let voting_module_addr = self.dao.voting_module_addr.clone();
        let denom = self.dao.x.denom.clone();
        self.execute_smart_ok(
            staker,
            voting_module_addr,
            &dao_voting_token_staked::msg::ExecuteMsg::Stake {},
            &coins(amount.into(), denom),
        );
    }

    /// unstake tokens
    pub fn unstake(&mut self, staker: impl Into<String>, amount: impl Into<Uint128>) {
        let voting_module_addr = self.dao.voting_module_addr.clone();
        self.execute_smart_ok(
            staker,
            &voting_module_addr,
            &dao_voting_token_staked::msg::ExecuteMsg::Unstake {
                amount: amount.into(),
            },
            &[],
        );
    }
}
