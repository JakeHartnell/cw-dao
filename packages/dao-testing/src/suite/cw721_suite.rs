use std::ops::{Deref, DerefMut};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, Addr, Binary, Empty};
use cw_utils::Duration;

use super::*;

#[cw_serde]
pub struct InitialNft {
    pub token_id: String,
    pub owner: String,
}

pub struct DaoTestingSuiteCw721<'a> {
    pub base: &'a mut DaoTestingSuiteBase,

    pub initial_nfts: Vec<InitialNft>,
    pub unstaking_duration: Option<Duration>,
    pub active_threshold: Option<dao_voting::threshold::ActiveThreshold>,
}

#[derive(Clone, Debug)]
pub struct Cw721DaoExtra {
    pub cw721_addr: Addr,
}

pub type Cw721TestDao = TestDao<Cw721DaoExtra>;

impl Deref for DaoTestingSuiteCw721<'_> {
    type Target = DaoTestingSuiteBase;

    fn deref(&self) -> &Self::Target {
        self.base
    }
}

impl DerefMut for DaoTestingSuiteCw721<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.base
    }
}

impl<'a> DaoTestingSuiteCw721<'a> {
    pub fn new(base: &'a mut DaoTestingSuiteBase) -> Self {
        Self {
            base,

            initial_nfts: vec![
                InitialNft {
                    token_id: "0".to_string(),
                    owner: ADDR0.to_string(),
                },
                InitialNft {
                    token_id: "1".to_string(),
                    owner: ADDR1.to_string(),
                },
                InitialNft {
                    token_id: "2".to_string(),
                    owner: ADDR2.to_string(),
                },
                InitialNft {
                    token_id: "3".to_string(),
                    owner: ADDR3.to_string(),
                },
                InitialNft {
                    token_id: "4".to_string(),
                    owner: ADDR4.to_string(),
                },
            ],
            unstaking_duration: None,
            active_threshold: None,
        }
    }

    pub fn with_initial_nfts(&mut self, initial_nfts: Vec<InitialNft>) -> &mut Self {
        self.initial_nfts = initial_nfts;
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

    /// stake NFT
    pub fn stake(
        &mut self,
        dao: &Cw721TestDao,
        staker: impl Into<String>,
        token_id: impl Into<String>,
    ) {
        self.execute_smart_ok(
            staker,
            &dao.x.cw721_addr,
            &cw721_base::msg::ExecuteMsg::<Empty, Empty>::SendNft {
                contract: dao.voting_module_addr.to_string(),
                token_id: token_id.into(),
                msg: Binary::default(),
            },
            &[],
        );
    }

    /// unstake NFT
    pub fn unstake(
        &mut self,
        dao: &Cw721TestDao,
        staker: impl Into<String>,
        token_id: impl Into<String>,
    ) {
        self.execute_smart_ok(
            staker,
            &dao.voting_module_addr,
            &dao_voting_cw721_staked::msg::ExecuteMsg::Unstake {
                token_ids: vec![token_id.into()],
            },
            &[],
        );
    }
}

impl DaoTestingSuite<Cw721DaoExtra> for DaoTestingSuiteCw721<'_> {
    fn base(&self) -> &DaoTestingSuiteBase {
        self
    }

    fn base_mut(&mut self) -> &mut DaoTestingSuiteBase {
        self
    }

    fn get_voting_module_info(&self) -> dao_interface::state::ModuleInstantiateInfo {
        dao_interface::state::ModuleInstantiateInfo {
            code_id: self.voting_cw721_staked_id,
            msg: to_json_binary(&dao_voting_cw721_staked::msg::InstantiateMsg {
                nft_contract: dao_voting_cw721_staked::msg::NftContract::New {
                    code_id: self.cw721_base_id,
                    label: "voting NFT".to_string(),
                    msg: to_json_binary(&cw721_base::msg::InstantiateMsg {
                        name: "Voting NFT".to_string(),
                        symbol: "VOTE".to_string(),
                        minter: OWNER.to_string(),
                    })
                    .unwrap(),
                    initial_nfts: self
                        .initial_nfts
                        .iter()
                        .map(|x| {
                            to_json_binary(&cw721_base::msg::ExecuteMsg::<Empty, Empty>::Mint {
                                token_id: x.token_id.clone(),
                                owner: x.owner.clone(),
                                token_uri: None,
                                extension: Empty {},
                            })
                            .unwrap()
                        })
                        .collect(),
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

    fn get_dao_extra(&self, dao: &TestDao) -> Cw721DaoExtra {
        let dao_voting_cw721_staked::state::Config { nft_address, .. } = self
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw721_staked::msg::QueryMsg::Config {},
            )
            .unwrap();

        Cw721DaoExtra {
            cw721_addr: nft_address,
        }
    }

    /// stake all initial NFTs and progress one block
    fn dao_setup(&mut self, dao: &mut Cw721TestDao) {
        for nft in self.initial_nfts.clone() {
            self.stake(dao, nft.owner, nft.token_id);
        }

        // staking takes effect at the next block
        self.advance_block();
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::Uint128;

    use super::*;

    #[test]
    fn dao_testing_suite_cw721() {
        let mut suite = DaoTestingSuiteBase::base();
        let mut suite = suite.cw721();
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

        let dao_voting_cw721_staked::state::Config { nft_address, .. } = suite
            .querier()
            .query_wasm_smart(
                &dao.voting_module_addr,
                &dao_voting_cw721_staked::msg::QueryMsg::Config {},
            )
            .unwrap();
        assert_eq!(nft_address, dao.x.cw721_addr);

        let total_weight: dao_interface::voting::TotalPowerAtHeightResponse = suite
            .querier()
            .query_wasm_smart(
                &dao.core_addr,
                &dao_interface::msg::QueryMsg::TotalPowerAtHeight { height: None },
            )
            .unwrap();
        assert_eq!(
            total_weight.power,
            Uint128::from(suite.initial_nfts.len() as u128)
        );
    }
}
