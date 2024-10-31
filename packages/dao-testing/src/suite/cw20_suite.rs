use cosmwasm_std::{to_json_binary, Addr, Uint128};
use cw20::Cw20Coin;
use cw_utils::Duration;

use super::*;

pub struct DaoTestingSuiteCw20<'a> {
    pub base: &'a mut DaoTestingSuiteBase,

    pub initial_balances: Vec<Cw20Coin>,
    pub initial_dao_balance: Uint128,
    pub unstaking_duration: Option<Duration>,
    pub active_threshold: Option<dao_voting::threshold::ActiveThreshold>,
}

#[derive(Clone, Debug)]
pub struct Cw20DaoExtra {
    pub cw20_addr: Addr,
    pub staking_addr: Addr,
}

pub type Cw20TestDao = TestDao<Cw20DaoExtra>;

impl<'a> DaoTestingSuiteCw20<'a> {
    pub fn new(base: &'a mut DaoTestingSuiteBase) -> Self {
        Self {
            base,

            initial_balances: vec![
                Cw20Coin {
                    address: MEMBER1.to_string(),
                    amount: Uint128::new(100),
                },
                Cw20Coin {
                    address: MEMBER2.to_string(),
                    amount: Uint128::new(200),
                },
                Cw20Coin {
                    address: MEMBER3.to_string(),
                    amount: Uint128::new(300),
                },
                Cw20Coin {
                    address: MEMBER4.to_string(),
                    amount: Uint128::new(300),
                },
                Cw20Coin {
                    address: MEMBER5.to_string(),
                    amount: Uint128::new(100),
                },
            ],
            initial_dao_balance: Uint128::new(10000),
            unstaking_duration: None,
            active_threshold: None,
        }
    }

    pub fn with_initial_balances(&mut self, initial_balances: Vec<Cw20Coin>) -> &mut Self {
        self.initial_balances = initial_balances;
        self
    }

    pub fn with_initial_dao_balance(
        &mut self,
        initial_dao_balance: impl Into<Uint128>,
    ) -> &mut Self {
        self.initial_dao_balance = initial_dao_balance.into();
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

    /// stake tokens
    pub fn stake(
        &mut self,
        dao: &Cw20TestDao,
        staker: impl Into<String>,
        amount: impl Into<Uint128>,
    ) {
        self.base
            .app
            .execute_contract(
                Addr::unchecked(staker),
                dao.x.cw20_addr.clone(),
                &cw20::Cw20ExecuteMsg::Send {
                    contract: dao.x.staking_addr.to_string(),
                    amount: amount.into(),
                    msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
                },
                &[],
            )
            .unwrap();
    }

    /// unstake tokens
    pub fn unstake(
        &mut self,
        dao: &Cw20TestDao,
        staker: impl Into<String>,
        amount: impl Into<Uint128>,
    ) {
        self.base
            .app
            .execute_contract(
                Addr::unchecked(staker),
                dao.x.staking_addr.clone(),
                &cw20_stake::msg::ExecuteMsg::Unstake {
                    amount: amount.into(),
                },
                &[],
            )
            .unwrap();
    }

    /// stake all initial balances and progress one block
    pub fn stake_all_initial(&mut self, dao: &Cw20TestDao) {
        for member in self.initial_balances.clone() {
            self.stake(dao, member.address, member.amount);
        }

        // staking takes effect at the next block
        self.base.advance_block();
    }
}

impl<'a> DaoTestingSuite<Cw20DaoExtra> for DaoTestingSuiteCw20<'a> {
    fn base(&self) -> &DaoTestingSuiteBase {
        self.base
    }

    fn base_mut(&mut self) -> &mut DaoTestingSuiteBase {
        self.base
    }

    fn get_voting_module_info(&self) -> dao_interface::state::ModuleInstantiateInfo {
        dao_interface::state::ModuleInstantiateInfo {
            code_id: self.base.voting_cw20_staked_id,
            msg: to_json_binary(&dao_voting_cw20_staked::msg::InstantiateMsg {
                token_info: dao_voting_cw20_staked::msg::TokenInfo::New {
                    code_id: self.base.cw20_base_id,
                    label: "voting token".to_string(),
                    name: "Voting Token".to_string(),
                    symbol: "VOTE".to_string(),
                    decimals: 6,
                    initial_balances: self.initial_balances.clone(),
                    marketing: None,
                    staking_code_id: self.base.cw20_stake_id,
                    unstaking_duration: self.unstaking_duration,
                    initial_dao_balance: Some(self.initial_dao_balance),
                },
                active_threshold: self.active_threshold.clone(),
            })
            .unwrap(),
            admin: Some(dao_interface::state::Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        }
    }

    fn get_dao_extra(&self, dao: &TestDao) -> Cw20DaoExtra {
        let cw20_addr: Addr = self
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw20_staked::msg::QueryMsg::TokenContract {},
            )
            .unwrap();
        let staking_addr: Addr = self
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
            )
            .unwrap();

        Cw20DaoExtra {
            cw20_addr,
            staking_addr,
        }
    }

    /// stake all initial balances and progress one block
    fn dao_setup(&mut self, dao: &mut Cw20TestDao) {
        for member in self.initial_balances.clone() {
            self.stake(dao, member.address, member.amount);
        }

        // staking takes effect at the next block
        self.base.advance_block();
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Uint128;

    use super::*;

    #[test]
    fn dao_testing_suite_cw20() {
        let mut suite = DaoTestingSuiteBase::base();
        let mut suite = suite.cw20();
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

        let cw20_addr: Addr = suite
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw20_staked::msg::QueryMsg::TokenContract {},
            )
            .unwrap();
        assert_eq!(cw20_addr, dao.x.cw20_addr);

        let staking_addr: Addr = suite
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw20_staked::msg::QueryMsg::StakingContract {},
            )
            .unwrap();
        assert_eq!(staking_addr, dao.x.staking_addr);

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
