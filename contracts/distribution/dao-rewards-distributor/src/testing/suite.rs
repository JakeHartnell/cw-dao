use cosmwasm_schema::cw_serde;
use cosmwasm_std::{coin, to_json_binary, Addr, Coin, Timestamp, Uint128};
use cw20::{Cw20Coin, Expiration, UncheckedDenom};
use cw4::{Member, MemberListResponse};
use cw_multi_test::{BankSudo, Executor, SudoMsg};
use cw_ownable::Action;
use cw_utils::Duration;
use dao_interface::{token::InitialBalance, voting::InfoResponse};
use dao_testing::{
    Cw20TestDao, Cw4TestDao, Cw721TestDao, DaoTestingSuite, DaoTestingSuiteBase, InitialNft,
    TokenTestDao, GOV_DENOM, MEMBER1, MEMBER2, MEMBER3, OWNER,
};

use crate::{
    msg::{
        CreateMsg, DistributionsResponse, ExecuteMsg, FundMsg, InstantiateMsg,
        PendingRewardsResponse, QueryMsg, ReceiveCw20Msg,
    },
    state::{DistributionState, EmissionRate},
};
use dao_rewards_distributor::ContractError;
pub enum DaoType {
    CW20,
    CW4,
    CW721,
    Native,
}

#[cw_serde]
pub struct RewardsConfig {
    pub amount: u128,
    pub denom: UncheckedDenom,
    pub duration: Duration,
    pub destination: Option<String>,
    pub continuous: bool,
}

pub struct SuiteBuilder {
    pub dao_type: DaoType,
    pub rewards_config: RewardsConfig,
    pub cw4_members: Vec<Member>,
}

impl SuiteBuilder {
    pub fn base(dao_type: DaoType) -> Self {
        Self {
            dao_type,
            rewards_config: RewardsConfig {
                amount: 1_000,
                denom: UncheckedDenom::Native(GOV_DENOM.to_string()),
                duration: Duration::Height(10),
                destination: None,
                continuous: true,
            },
            cw4_members: vec![
                Member {
                    addr: MEMBER1.to_string(),
                    weight: 2,
                },
                Member {
                    addr: MEMBER2.to_string(),
                    weight: 1,
                },
                Member {
                    addr: MEMBER3.to_string(),
                    weight: 1,
                },
            ],
        }
    }

    pub fn with_rewards_config(mut self, rewards_config: RewardsConfig) -> Self {
        self.rewards_config = rewards_config;
        self
    }

    pub fn with_cw4_members(mut self, cw4_members: Vec<Member>) -> Self {
        self.cw4_members = cw4_members;
        self
    }

    pub fn with_withdraw_destination(mut self, withdraw_destination: Option<String>) -> Self {
        self.rewards_config.destination = withdraw_destination;
        self
    }
}

impl SuiteBuilder {
    pub fn build(mut self) -> Suite {
        let mut suite_built = Suite {
            base: DaoTestingSuiteBase::base(),
            core_addr: Addr::unchecked(""),
            staking_addr: Addr::unchecked(""),
            voting_power_addr: Addr::unchecked(""),
            reward_code_id: 0,
            distribution_contract: Addr::unchecked(""),
            cw20_addr: Addr::unchecked(""),
            reward_denom: GOV_DENOM.to_string(),
            cw20_dao: None,
            cw4_dao: None,
            cw721_dao: None,
            token_dao: None,
        };

        match self.dao_type {
            DaoType::CW4 => {
                let dao = suite_built.base.cw4().with_members(self.cw4_members).dao();
                suite_built.core_addr = dao.core_addr.clone();
                suite_built.voting_power_addr = dao.voting_module_addr.clone();
                suite_built.staking_addr = dao.x.group_addr.clone();
                suite_built.cw4_dao = Some(dao);
            }
            DaoType::CW20 => {
                let dao = suite_built
                    .base
                    .cw20()
                    .with_initial_balances(vec![
                        Cw20Coin {
                            address: MEMBER1.to_string(),
                            amount: Uint128::new(100),
                        },
                        Cw20Coin {
                            address: MEMBER2.to_string(),
                            amount: Uint128::new(50),
                        },
                        Cw20Coin {
                            address: MEMBER3.to_string(),
                            amount: Uint128::new(50),
                        },
                    ])
                    .dao();

                suite_built.core_addr = dao.core_addr.clone();
                suite_built.voting_power_addr = dao.voting_module_addr.clone();
                suite_built.cw20_addr = dao.x.cw20_addr.clone();
                suite_built.staking_addr = dao.x.staking_addr.clone();
                suite_built.cw20_dao = Some(dao);
            }
            DaoType::CW721 => {
                let dao = suite_built
                    .base
                    .cw721()
                    .with_initial_nfts(vec![
                        InitialNft {
                            token_id: "1".to_string(),
                            owner: MEMBER1.to_string(),
                        },
                        InitialNft {
                            token_id: "2".to_string(),
                            owner: MEMBER1.to_string(),
                        },
                        InitialNft {
                            token_id: "3".to_string(),
                            owner: MEMBER2.to_string(),
                        },
                        InitialNft {
                            token_id: "4".to_string(),
                            owner: MEMBER3.to_string(),
                        },
                    ])
                    .dao();

                suite_built.core_addr = dao.core_addr.clone();
                suite_built.voting_power_addr = dao.voting_module_addr.clone();
                suite_built.staking_addr = dao.x.cw721_addr.clone();
                suite_built.cw721_dao = Some(dao);
            }
            DaoType::Native => {
                let dao = suite_built
                    .base
                    .token()
                    .with_initial_balances(vec![
                        InitialBalance {
                            address: MEMBER1.to_string(),
                            amount: Uint128::new(100),
                        },
                        InitialBalance {
                            address: MEMBER2.to_string(),
                            amount: Uint128::new(50),
                        },
                        InitialBalance {
                            address: MEMBER3.to_string(),
                            amount: Uint128::new(50),
                        },
                    ])
                    .dao();

                suite_built.core_addr = dao.core_addr.clone();
                suite_built.voting_power_addr = dao.voting_module_addr.clone();
                suite_built.staking_addr = dao.voting_module_addr.clone();
                suite_built.token_dao = Some(dao);
            }
        };

        // start at 0 height and time
        suite_built.base.app.update_block(|b| {
            b.height = 0;
            b.time = Timestamp::from_seconds(0);
        });

        // initialize the rewards distributor
        suite_built.reward_code_id = suite_built.base.rewards_distributor_id;
        suite_built.distribution_contract = suite_built
            .base
            .app
            .instantiate_contract(
                suite_built.reward_code_id,
                Addr::unchecked(OWNER),
                &InstantiateMsg {
                    owner: Some(OWNER.to_string()),
                },
                &[],
                "reward",
                None,
            )
            .unwrap();

        // depending on the dao type we register rewards differently
        match self.dao_type {
            DaoType::CW721 => {
                suite_built.register_hook(suite_built.voting_power_addr.clone());
                suite_built.create(
                    self.rewards_config.clone(),
                    suite_built.voting_power_addr.to_string().as_ref(),
                    None,
                );
                match self.rewards_config.denom {
                    UncheckedDenom::Native(_) => {
                        suite_built.fund_native(1, coin(100_000_000, GOV_DENOM.to_string()));
                    }
                    UncheckedDenom::Cw20(_) => {
                        suite_built.fund_cw20(
                            1,
                            Cw20Coin {
                                address: suite_built.cw20_addr.to_string(),
                                amount: Uint128::new(100_000_000),
                            },
                        );
                    }
                };
            }
            _ => {
                self.rewards_config.denom = match self.rewards_config.denom {
                    UncheckedDenom::Native(denom) => UncheckedDenom::Native(denom),
                    UncheckedDenom::Cw20(_) => UncheckedDenom::Cw20(
                        suite_built
                            .base
                            .instantiate_cw20(
                                "rewardcw",
                                vec![Cw20Coin {
                                    address: OWNER.to_string(),
                                    amount: Uint128::new(1_000_000_000),
                                }],
                            )
                            .to_string(),
                    ),
                };
                suite_built.reward_denom = match self.rewards_config.denom.clone() {
                    UncheckedDenom::Native(denom) => denom,
                    UncheckedDenom::Cw20(addr) => addr,
                };

                suite_built.register_hook(suite_built.staking_addr.clone());
                suite_built.create(
                    self.rewards_config.clone(),
                    suite_built.staking_addr.to_string().as_ref(),
                    None,
                );
                match &self.rewards_config.denom {
                    UncheckedDenom::Native(_) => {
                        suite_built.fund_native(1, coin(100_000_000, GOV_DENOM.to_string()));
                    }
                    UncheckedDenom::Cw20(addr) => {
                        suite_built.fund_cw20(
                            1,
                            Cw20Coin {
                                address: addr.to_string(),
                                amount: Uint128::new(100_000_000),
                            },
                        );
                    }
                };
            }
        }

        println!("voting power addr: {}", suite_built.voting_power_addr);
        println!("staking addr: {}", suite_built.staking_addr);
        suite_built
    }
}

pub struct Suite {
    pub base: DaoTestingSuiteBase,

    pub core_addr: Addr,
    pub staking_addr: Addr,
    pub voting_power_addr: Addr,
    pub reward_denom: String,

    pub reward_code_id: u64,
    pub distribution_contract: Addr,

    // cw20 type fields
    pub cw20_addr: Addr,

    // DAO types
    pub cw20_dao: Option<Cw20TestDao>,
    pub cw4_dao: Option<Cw4TestDao>,
    pub cw721_dao: Option<Cw721TestDao>,
    pub token_dao: Option<TokenTestDao>,
}

// SUITE QUERIES
impl Suite {
    pub fn get_time_until_rewards_expiration(&mut self) -> u64 {
        let distribution = &self.get_distributions().distributions[0];
        let current_block = self.base.app.block_info();
        let (expiration_unit, current_unit) = match distribution.active_epoch.ends_at {
            cw20::Expiration::AtHeight(h) => (h, current_block.height),
            cw20::Expiration::AtTime(t) => (t.seconds(), current_block.time.seconds()),
            cw20::Expiration::Never {} => return 0,
        };

        if expiration_unit > current_unit {
            expiration_unit - current_unit
        } else {
            0
        }
    }

    pub fn get_balance_native<T: Into<String>, U: Into<String>>(
        &self,
        address: T,
        denom: U,
    ) -> u128 {
        self.base
            .app
            .wrap()
            .query_balance(address, denom)
            .unwrap()
            .amount
            .u128()
    }

    pub fn get_balance_cw20<T: Into<String>, U: Into<String>>(
        &self,
        contract_addr: T,
        address: U,
    ) -> u128 {
        let msg = cw20::Cw20QueryMsg::Balance {
            address: address.into(),
        };
        let result: cw20::BalanceResponse = self
            .base
            .app
            .wrap()
            .query_wasm_smart(contract_addr, &msg)
            .unwrap();
        result.balance.u128()
    }

    pub fn get_distributions(&mut self) -> DistributionsResponse {
        self.base
            .app
            .wrap()
            .query_wasm_smart(
                self.distribution_contract.clone(),
                &QueryMsg::Distributions {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
    }

    pub fn get_distribution(&mut self, id: u64) -> DistributionState {
        let resp: DistributionState = self
            .base
            .app
            .wrap()
            .query_wasm_smart(
                self.distribution_contract.clone(),
                &QueryMsg::Distribution { id },
            )
            .unwrap();
        resp
    }

    pub fn get_undistributed_rewards(&mut self, id: u64) -> Uint128 {
        let undistributed_rewards: Uint128 = self
            .base
            .app
            .wrap()
            .query_wasm_smart(
                self.distribution_contract.clone(),
                &QueryMsg::UndistributedRewards { id },
            )
            .unwrap();
        undistributed_rewards
    }

    pub fn get_owner(&mut self) -> Addr {
        let ownable_response: cw_ownable::Ownership<Addr> = self
            .base
            .app
            .wrap()
            .query_wasm_smart(self.distribution_contract.clone(), &QueryMsg::Ownership {})
            .unwrap();
        ownable_response.owner.unwrap()
    }

    pub fn get_info(&mut self) -> InfoResponse {
        self.base
            .app
            .wrap()
            .query_wasm_smart(self.distribution_contract.clone(), &QueryMsg::Info {})
            .unwrap()
    }
}

// SUITE ASSERTIONS
impl Suite {
    pub fn assert_ends_at(&mut self, expected: Expiration) {
        let distribution = &self.get_distributions().distributions[0];
        assert_eq!(distribution.active_epoch.ends_at, expected);
    }

    pub fn assert_started_at(&mut self, expected: Expiration) {
        let distribution = &self.get_distributions().distributions[0];
        assert_eq!(distribution.active_epoch.started_at, expected);
    }

    pub fn assert_amount(&mut self, expected: u128) {
        let distribution = &self.get_distributions().distributions[0];
        match distribution.active_epoch.emission_rate {
            EmissionRate::Paused {} => panic!("expected non-paused emission rate"),
            EmissionRate::Immediate {} => panic!("expected non-immediate emission rate"),
            EmissionRate::Linear { amount, .. } => assert_eq!(amount, Uint128::new(expected)),
        }
    }

    pub fn assert_duration(&mut self, expected: u64) {
        let distribution = &self.get_distributions().distributions[0];
        match distribution.active_epoch.emission_rate {
            EmissionRate::Paused {} => panic!("expected non-paused emission rate"),
            EmissionRate::Immediate {} => panic!("expected non-immediate emission rate"),
            EmissionRate::Linear { duration, .. } => assert_eq!(
                match duration {
                    Duration::Height(h) => h,
                    Duration::Time(t) => t,
                },
                expected
            ),
        }
    }

    pub fn assert_pending_rewards(&mut self, address: &str, id: u64, expected: u128) {
        let res: PendingRewardsResponse = self
            .base
            .app
            .wrap()
            .query_wasm_smart(
                self.distribution_contract.clone(),
                &QueryMsg::PendingRewards {
                    address: address.to_string(),
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();

        let pending = res
            .pending_rewards
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .pending_rewards;

        assert_eq!(
            pending,
            &Uint128::new(expected),
            "expected {} pending rewards, got {}",
            expected,
            pending
        );
    }

    pub fn assert_undistributed_rewards(&mut self, id: u64, expected: u128) {
        let undistributed_rewards = self.get_undistributed_rewards(id);
        assert_eq!(
            undistributed_rewards,
            &Uint128::new(expected),
            "expected {} undistributed rewards, got {}",
            expected,
            undistributed_rewards
        );
    }

    pub fn assert_native_balance(&self, address: &str, denom: &str, expected: u128) {
        let balance = self.get_balance_native(address, denom);
        assert_eq!(balance, expected);
    }

    pub fn assert_cw20_balance(&self, cw20: &str, address: &str, expected: u128) {
        let balance = self.get_balance_cw20(cw20, address);
        assert_eq!(balance, expected);
    }
}

// SUITE ACTIONS
impl Suite {
    pub fn withdraw(&mut self, id: u64) {
        let msg = ExecuteMsg::Withdraw { id };
        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn withdraw_error(&mut self, id: u64) -> ContractError {
        let msg = ExecuteMsg::Withdraw { id };
        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap_err()
            .downcast()
            .unwrap()
    }

    pub fn register_hook(&mut self, addr: Addr) {
        let msg = cw4_group::msg::ExecuteMsg::AddHook {
            addr: self.distribution_contract.to_string(),
        };
        self.base
            .app
            .execute_contract(self.core_addr.clone(), addr, &msg, &[])
            .unwrap();
    }

    pub fn create(
        &mut self,
        reward_config: RewardsConfig,
        hook_caller: &str,
        funds: Option<Uint128>,
    ) {
        let execute_create_msg = ExecuteMsg::Create(CreateMsg {
            denom: reward_config.denom.clone(),
            emission_rate: EmissionRate::Linear {
                amount: Uint128::new(reward_config.amount),
                duration: reward_config.duration,
                continuous: reward_config.continuous,
            },
            hook_caller: hook_caller.to_string(),
            vp_contract: self.voting_power_addr.to_string(),
            open_funding: None,
            withdraw_destination: reward_config.destination,
        });

        // include funds if provided
        let send_funds = if let Some(funds) = funds {
            match reward_config.denom {
                UncheckedDenom::Native(denom) => vec![coin(funds.u128(), denom)],
                UncheckedDenom::Cw20(_) => vec![],
            }
        } else {
            vec![]
        };

        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &execute_create_msg,
                &send_funds,
            )
            .unwrap();
    }

    pub fn mint_native(&mut self, coin: Coin, dest: &str) {
        // mint the tokens to be funded
        self.base
            .app
            .sudo(SudoMsg::Bank({
                BankSudo::Mint {
                    to_address: dest.to_string(),
                    amount: vec![coin.clone()],
                }
            }))
            .unwrap();
    }

    pub fn mint_cw20(&mut self, coin: Cw20Coin, name: &str) -> Addr {
        self.base.instantiate_cw20(name, vec![coin])
    }

    pub fn fund_native(&mut self, id: u64, coin: Coin) {
        self.mint_native(coin.clone(), OWNER);
        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &ExecuteMsg::Fund(FundMsg { id }),
                &[coin],
            )
            .unwrap();
    }

    pub fn fund_latest_native(&mut self, coin: Coin) {
        self.mint_native(coin.clone(), OWNER);
        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &ExecuteMsg::FundLatest {},
                &[coin],
            )
            .unwrap();
    }

    pub fn fund_cw20(&mut self, id: u64, coin: Cw20Coin) {
        let fund_sub_msg = to_json_binary(&ReceiveCw20Msg::Fund(FundMsg { id })).unwrap();
        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                Addr::unchecked(coin.address),
                &cw20::Cw20ExecuteMsg::Send {
                    contract: self.distribution_contract.to_string(),
                    amount: coin.amount,
                    msg: fund_sub_msg,
                },
                &[],
            )
            .unwrap();
    }

    pub fn fund_latest_cw20(&mut self, coin: Cw20Coin) {
        let fund_sub_msg = to_json_binary(&ReceiveCw20Msg::FundLatest {}).unwrap();
        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                Addr::unchecked(coin.address),
                &cw20::Cw20ExecuteMsg::Send {
                    contract: self.distribution_contract.to_string(),
                    amount: coin.amount,
                    msg: fund_sub_msg,
                },
                &[],
            )
            .unwrap();
    }

    pub fn skip_blocks(&mut self, blocks: u64) {
        self.base.app.update_block(|b| {
            println!("skipping blocks {:?} -> {:?}", b.height, b.height + blocks);
            b.height += blocks
        });
    }

    pub fn skip_seconds(&mut self, seconds: u64) {
        self.base.app.update_block(|b| {
            let new_block_time = b.time.plus_seconds(seconds);
            println!(
                "skipping seconds {:?} -> {:?}",
                b.time.seconds(),
                new_block_time.seconds()
            );
            b.time = new_block_time;
            // this is needed because voting power query only exists based on height.
            // for time-based unit tests we assume that 1 block = 1 second.
            // only implication I can think of is that during mainnet network downtime,
            // rewards would continue to accrue for time-based distributions, whereas
            // height-based distributions would not.
            b.height += seconds;
        });
    }

    pub fn claim_rewards(&mut self, address: &str, id: u64) {
        let msg = ExecuteMsg::Claim { id };
        self.base
            .app
            .execute_contract(
                Addr::unchecked(address),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    #[allow(dead_code)]
    pub fn stake_cw20_tokens(&mut self, amount: u128, sender: &str) {
        let msg = cw20::Cw20ExecuteMsg::Send {
            contract: self.staking_addr.to_string(),
            amount: Uint128::new(amount),
            msg: to_json_binary(&cw20_stake::msg::ReceiveMsg::Stake {}).unwrap(),
        };
        self.base
            .app
            .execute_contract(Addr::unchecked(sender), self.cw20_addr.clone(), &msg, &[])
            .unwrap();
    }

    pub fn unstake_cw20_tokens(&mut self, amount: u128, sender: &str) {
        let msg = cw20_stake::msg::ExecuteMsg::Unstake {
            amount: Uint128::new(amount),
        };
        self.base
            .app
            .execute_contract(
                Addr::unchecked(sender),
                self.staking_addr.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn stake_nft(&mut self, sender: &str, token_id: u64) {
        self.base.cw721().stake(
            &self.cw721_dao.clone().unwrap(),
            sender,
            token_id.to_string(),
        )
    }

    pub fn unstake_nft(&mut self, sender: &str, token_id: u64) {
        self.base.cw721().unstake(
            &self.cw721_dao.clone().unwrap(),
            sender,
            token_id.to_string(),
        )
    }

    pub fn stake_native_tokens(&mut self, address: &str, amount: u128) {
        self.base
            .token()
            .stake(&self.token_dao.clone().unwrap(), address, amount)
    }

    pub fn unstake_native_tokens(&mut self, address: &str, amount: u128) {
        self.base
            .token()
            .unstake(&self.token_dao.clone().unwrap(), address, amount)
    }

    pub fn update_emission_rate(
        &mut self,
        id: u64,
        epoch_duration: Duration,
        epoch_rewards: u128,
        continuous: bool,
    ) {
        let msg: ExecuteMsg = ExecuteMsg::Update {
            id,
            emission_rate: Some(EmissionRate::Linear {
                amount: Uint128::new(epoch_rewards),
                duration: epoch_duration,
                continuous,
            }),
            vp_contract: None,
            hook_caller: None,
            open_funding: None,
            withdraw_destination: None,
        };

        let _resp = self
            .base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn set_immediate_emission(&mut self, id: u64) {
        let msg: ExecuteMsg = ExecuteMsg::Update {
            id,
            emission_rate: Some(EmissionRate::Immediate {}),
            vp_contract: None,
            hook_caller: None,
            open_funding: None,
            withdraw_destination: None,
        };

        let _resp = self
            .base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn pause_emission(&mut self, id: u64) {
        let msg: ExecuteMsg = ExecuteMsg::Update {
            id,
            emission_rate: Some(EmissionRate::Paused {}),
            vp_contract: None,
            hook_caller: None,
            open_funding: None,
            withdraw_destination: None,
        };

        let _resp = self
            .base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn update_vp_contract(&mut self, id: u64, vp_contract: &str) {
        let msg: ExecuteMsg = ExecuteMsg::Update {
            id,
            emission_rate: None,
            vp_contract: Some(vp_contract.to_string()),
            hook_caller: None,
            open_funding: None,
            withdraw_destination: None,
        };

        let _resp = self
            .base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn update_hook_caller(&mut self, id: u64, hook_caller: &str) {
        let msg: ExecuteMsg = ExecuteMsg::Update {
            id,
            emission_rate: None,
            vp_contract: None,
            hook_caller: Some(hook_caller.to_string()),
            open_funding: None,
            withdraw_destination: None,
        };

        let _resp = self
            .base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn update_open_funding(&mut self, id: u64, open_funding: bool) {
        let msg: ExecuteMsg = ExecuteMsg::Update {
            id,
            emission_rate: None,
            vp_contract: None,
            hook_caller: None,
            open_funding: Some(open_funding),
            withdraw_destination: None,
        };

        let _resp = self
            .base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn update_withdraw_destination(&mut self, id: u64, withdraw_destination: &str) {
        let msg: ExecuteMsg = ExecuteMsg::Update {
            id,
            emission_rate: None,
            vp_contract: None,
            hook_caller: None,
            open_funding: None,
            withdraw_destination: Some(withdraw_destination.to_string()),
        };

        let _resp = self
            .base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn update_members(&mut self, add: Vec<Member>, remove: Vec<String>) {
        let msg = cw4_group::msg::ExecuteMsg::UpdateMembers { remove, add };

        self.base
            .app
            .execute_contract(self.core_addr.clone(), self.staking_addr.clone(), &msg, &[])
            .unwrap();
    }

    pub fn query_members(&mut self) -> Vec<Member> {
        let members: MemberListResponse = self
            .base
            .app
            .wrap()
            .query_wasm_smart(
                self.staking_addr.clone(),
                &cw4_group::msg::QueryMsg::ListMembers {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap();
        // println!("[UPDATE CW4] new members: {:?}", members);
        members.members
    }

    pub fn update_owner(&mut self, new_owner: &str) {
        let msg = ExecuteMsg::UpdateOwnership(Action::TransferOwnership {
            new_owner: new_owner.to_string(),
            expiry: None,
        });

        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();

        self.base
            .app
            .execute_contract(
                Addr::unchecked(new_owner),
                self.distribution_contract.clone(),
                &ExecuteMsg::UpdateOwnership(Action::AcceptOwnership {}),
                &[],
            )
            .unwrap();
    }

    pub fn unsafe_force_withdraw(&mut self, amount: Coin) {
        let msg = ExecuteMsg::UnsafeForceWithdraw { amount };
        self.base
            .app
            .execute_contract(
                Addr::unchecked(OWNER),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap();
    }

    pub fn unsafe_force_withdraw_unauthorized(&mut self, amount: Coin) -> ContractError {
        let msg = ExecuteMsg::UnsafeForceWithdraw { amount };
        self.base
            .app
            .execute_contract(
                Addr::unchecked("no_one"),
                self.distribution_contract.clone(),
                &msg,
                &[],
            )
            .unwrap_err()
            .downcast()
            .unwrap()
    }
}
