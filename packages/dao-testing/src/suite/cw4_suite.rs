use std::ops::{Deref, DerefMut};

use cosmwasm_std::{to_json_binary, Addr};

use super::*;

pub struct DaoTestingSuiteCw4<'a> {
    pub base: &'a mut DaoTestingSuiteBase,

    pub members: Vec<cw4::Member>,
}

#[derive(Clone, Debug)]
pub struct Cw4DaoExtra {
    pub group_addr: Addr,
}

pub type Cw4TestDao = TestDao<Cw4DaoExtra>;

impl Deref for DaoTestingSuiteCw4<'_> {
    type Target = DaoTestingSuiteBase;

    fn deref(&self) -> &Self::Target {
        self.base
    }
}

impl DerefMut for DaoTestingSuiteCw4<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.base
    }
}

impl<'a> DaoTestingSuiteCw4<'a> {
    pub fn new(base: &'a mut DaoTestingSuiteBase) -> Self {
        Self {
            base,
            members: vec![
                cw4::Member {
                    addr: ADDR0.to_string(),
                    weight: 1,
                },
                cw4::Member {
                    addr: ADDR1.to_string(),
                    weight: 2,
                },
                cw4::Member {
                    addr: ADDR2.to_string(),
                    weight: 3,
                },
                cw4::Member {
                    addr: ADDR3.to_string(),
                    weight: 3,
                },
                cw4::Member {
                    addr: ADDR4.to_string(),
                    weight: 1,
                },
            ],
        }
    }

    pub fn with_members(&mut self, members: Vec<cw4::Member>) -> &mut Self {
        self.members = members;
        self
    }
}

impl DaoTestingSuite<Cw4DaoExtra> for DaoTestingSuiteCw4<'_> {
    fn base(&self) -> &DaoTestingSuiteBase {
        self
    }

    fn base_mut(&mut self) -> &mut DaoTestingSuiteBase {
        self
    }

    fn get_voting_module_info(&self) -> dao_interface::state::ModuleInstantiateInfo {
        dao_interface::state::ModuleInstantiateInfo {
            code_id: self.voting_cw4_id,
            msg: to_json_binary(&dao_voting_cw4::msg::InstantiateMsg {
                group_contract: dao_voting_cw4::msg::GroupContract::New {
                    cw4_group_code_id: self.cw4_group_id,
                    initial_members: self.members.clone(),
                },
            })
            .unwrap(),
            admin: Some(dao_interface::state::Admin::CoreModule {}),
            funds: vec![],
            label: "voting module".to_string(),
        }
    }

    fn get_dao_extra(&self, dao: &TestDao) -> Cw4DaoExtra {
        let group_addr: Addr = self
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw4::msg::QueryMsg::GroupContract {},
            )
            .unwrap();

        Cw4DaoExtra { group_addr }
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Uint128;

    use super::*;

    #[test]
    fn dao_testing_suite_cw4() {
        let mut base = DaoTestingSuiteBase::base();
        let mut suite = base.cw4();
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

        let group_addr: Addr = suite
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw4::msg::QueryMsg::GroupContract {},
            )
            .unwrap();
        assert_eq!(group_addr, dao.x.group_addr);

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
                .members
                .iter()
                .fold(Uint128::zero(), |acc, m| acc + Uint128::from(m.weight))
        );
    }
}
