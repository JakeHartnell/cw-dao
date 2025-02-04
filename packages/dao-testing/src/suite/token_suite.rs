use std::ops::{Deref, DerefMut};

use cosmwasm_std::{coins, to_json_binary, Uint128};
use cw_multi_test::{BankSudo, SudoMsg};
use cw_utils::Duration;
use dao_interface::token::InitialBalance;

use super::*;

pub struct DaoTestingSuiteToken<'a> {
    pub base: &'a mut DaoTestingSuiteBase,

    pub initial_balances: Vec<InitialBalance>,
    pub unstaking_duration: Option<Duration>,
    pub active_threshold: Option<dao_voting::threshold::ActiveThreshold>,
}

#[derive(Clone, Debug)]
pub struct TokenDaoExtra {
    pub denom: String,
}

pub type TokenTestDao = TestDao<TokenDaoExtra>;

impl Deref for DaoTestingSuiteToken<'_> {
    type Target = DaoTestingSuiteBase;

    fn deref(&self) -> &Self::Target {
        self.base
    }
}

impl DerefMut for DaoTestingSuiteToken<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.base
    }
}

impl<'a> DaoTestingSuiteToken<'a> {
    pub fn new(base: &'a mut DaoTestingSuiteBase) -> Self {
        Self {
            base,

            initial_balances: vec![
                InitialBalance {
                    address: ADDR0.to_string(),
                    amount: Uint128::new(100),
                },
                InitialBalance {
                    address: ADDR1.to_string(),
                    amount: Uint128::new(200),
                },
                InitialBalance {
                    address: ADDR2.to_string(),
                    amount: Uint128::new(300),
                },
                InitialBalance {
                    address: ADDR3.to_string(),
                    amount: Uint128::new(300),
                },
                InitialBalance {
                    address: ADDR4.to_string(),
                    amount: Uint128::new(100),
                },
            ],
            unstaking_duration: None,
            active_threshold: None,
        }
    }

    pub fn with_initial_balances(&mut self, initial_balances: Vec<InitialBalance>) -> &mut Self {
        self.initial_balances = initial_balances;
        self
    }

    pub fn with_unstaking_duration(&mut self, unstaking_duration: Option<Duration>) -> &mut Self {
        self.unstaking_duration = unstaking_duration;
        self
    }

    pub fn with_active_threshold(
        &mut self,
        active_threshold: Option<dao_voting::threshold::ActiveThreshold>,
    ) -> &mut Self {
        self.active_threshold = active_threshold;
        self
    }

    /// mint tokens
    pub fn mint(
        &mut self,
        dao: &TokenTestDao,
        recipient: impl Into<String>,
        amount: impl Into<u128>,
    ) {
        self.app
            .sudo(SudoMsg::Bank({
                BankSudo::Mint {
                    to_address: recipient.into(),
                    amount: coins(amount.into(), &dao.x.denom),
                }
            }))
            .unwrap();
    }

    /// stake tokens
    pub fn stake(
        &mut self,
        dao: &TokenTestDao,
        staker: impl Into<String>,
        amount: impl Into<u128>,
    ) {
        self.execute_smart_ok(
            staker,
            &dao.voting_module_addr,
            &dao_voting_token_staked::msg::ExecuteMsg::Stake {},
            &coins(amount.into(), &dao.x.denom),
        );
    }

    /// unstake tokens
    pub fn unstake(
        &mut self,
        dao: &TokenTestDao,
        staker: impl Into<String>,
        amount: impl Into<Uint128>,
    ) {
        self.execute_smart_ok(
            staker,
            &dao.voting_module_addr,
            &dao_voting_token_staked::msg::ExecuteMsg::Unstake {
                amount: amount.into(),
            },
            &[],
        );
    }
}

impl DaoTestingSuite<TokenDaoExtra> for DaoTestingSuiteToken<'_> {
    fn base(&self) -> &DaoTestingSuiteBase {
        self
    }

    fn base_mut(&mut self) -> &mut DaoTestingSuiteBase {
        self
    }

    fn get_voting_module_info(&self) -> dao_interface::state::ModuleInstantiateInfo {
        dao_interface::state::ModuleInstantiateInfo {
            code_id: self.voting_token_staked_id,
            msg: to_json_binary(&dao_voting_token_staked::msg::InstantiateMsg {
                token_info: dao_voting_token_staked::msg::TokenInfo::Existing {
                    denom: GOV_DENOM.to_string(),
                },
                unstaking_duration: self.unstaking_duration,
                active_threshold: self.active_threshold.clone(),
            })
            .unwrap(),
            admin: Some(dao_interface::state::Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        }
    }

    fn get_dao_extra(&self, dao: &TestDao) -> TokenDaoExtra {
        let dao_interface::voting::DenomResponse { denom } = self
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_token_staked::msg::QueryMsg::Denom {},
            )
            .unwrap();

        TokenDaoExtra { denom }
    }

    /// mint and stake all initial balances and progress one block
    fn dao_setup(&mut self, dao: &mut TokenTestDao) {
        for member in self.initial_balances.clone() {
            self.mint(dao, member.address.clone(), member.amount);
            self.stake(dao, member.address, member.amount);
        }

        // staking takes effect at the next block
        self.advance_block();
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{Addr, Uint128};

    use super::*;

    #[test]
    fn dao_testing_suite_token() {
        let mut suite = DaoTestingSuiteBase::base();
        let mut suite = suite.token();
        let dao = suite.dao();

        let voting_module: Addr = suite
            .querier()
            .query_wasm_smart(
                &dao.core_addr,
                &dao_interface::msg::QueryMsg::VotingModule {},
            )
            .unwrap();
        assert_eq!(voting_module, dao.voting_module_addr);

        let proposal_modules: Vec<dao_interface::state::ProposalModule> = suite
            .querier()
            .query_wasm_smart(
                &dao.core_addr,
                &dao_interface::msg::QueryMsg::ProposalModules {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        assert_eq!(proposal_modules.len(), 2);

        let total_weight: dao_interface::voting::TotalPowerAtHeightResponse = suite
            .querier()
            .query_wasm_smart(
                &dao.core_addr,
                &dao_interface::msg::QueryMsg::TotalPowerAtHeight { height: None },
            )
            .unwrap();
        assert_eq!(
            total_weight.power,
            suite
                .initial_balances
                .iter()
                .fold(Uint128::zero(), |acc, m| acc + m.amount)
        );
    }
}
