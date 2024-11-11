use crate::msg::AssetUnchecked;
use crate::msg::CheckOptionResponse;
use crate::msg::{
    AdapterQueryMsg, AllOptionsResponse, AllSubmissionsResponse, ExecuteMsg, ReceiveMsg,
    SubmissionResponse,
};
use anyhow::Result as AnyResult;
use cosmwasm_std::{to_json_binary, Addr, Binary, Coin, Uint128};
use cw20::{BalanceResponse, Cw20QueryMsg};
use cw20::{Cw20Coin, MinterResponse};
use cw20_base::msg::ExecuteMsg as Cw20BaseExecuteMsg;
use cw20_base::msg::InstantiateMsg as Cw20BaseInstantiateMsg;

use cw_denom::UncheckedDenom;
use cw_multi_test::{App, AppResponse, ContractWrapper, Executor};

pub const NATIVE: &str = "juno";
pub const CW20: &str = "wynd";
pub const OWNER: &str = "owner";

// Store the marketing gauge adapter contract and returns the code id.
fn store_gauge_adapter(app: &mut App) -> u64 {
    let contract = Box::new(ContractWrapper::new_with_empty(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    ));

    app.store_code(contract)
}

// Store the cw20 contract and returns the code id.
fn store_cw20(app: &mut App) -> u64 {
    let contract = Box::new(ContractWrapper::new_with_empty(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    ));

    app.store_code(contract)
}

#[derive(Debug)]
pub struct SuiteBuilder {
    // Gauge adapter's instantiate params
    community_pool: String,
    required_deposit: Option<AssetUnchecked>,
    reward: AssetUnchecked,
    funds: Vec<(Addr, Vec<Coin>)>,
    cw20_funds: Vec<Cw20Coin>,
}

impl SuiteBuilder {
    pub fn new() -> Self {
        Self {
            community_pool: "community".to_owned(),
            required_deposit: None,
            reward: AssetUnchecked {
                denom: UncheckedDenom::Native(NATIVE.into()),
                amount: Uint128::new(1_000_000),
            },
            funds: vec![],
            cw20_funds: vec![],
        }
    }

    pub fn with_community_pool(mut self, community_pool: &str) -> Self {
        self.community_pool = community_pool.to_string();
        self
    }

    // Allows to initialize the suite with native coins associated to an address.
    pub fn with_funds(mut self, addr: &str, funds: &[Coin]) -> Self {
        self.funds.push((Addr::unchecked(addr), funds.into()));
        self
    }

    // Allows to initialize the suite with default cw20 tokens associated to an address.
    pub fn with_cw20_funds(mut self, addr: &str, amount: u128) -> Self {
        self.cw20_funds.push(Cw20Coin {
            address: addr.into(),
            amount: Uint128::from(amount),
        });
        self
    }

    // Allows to initialize the marketing gauge adapter with required native coins in the config.
    pub fn with_native_deposit(mut self, amount: u128) -> Self {
        self.required_deposit = Some(AssetUnchecked {
            denom: UncheckedDenom::Native(NATIVE.into()),
            amount: Uint128::from(amount),
        });
        self
    }

    // Allows to initialize the marketing gauge adapter with required cw20 tokens in the config.
    pub fn with_cw20_deposit(mut self, amount: u128) -> Self {
        self.required_deposit = Some(AssetUnchecked {
            denom: UncheckedDenom::Cw20("contract0".into()),
            amount: Uint128::from(amount),
        });
        self
    }

    // Instantiate a marketing gauge adapter and returns its address.
    fn instantiate_marketing_gauge_adapter(
        &self,
        app: &mut App,
        gauge_id: u64,
        owner: &str,
    ) -> Addr {
        app.instantiate_contract(
            gauge_id,
            Addr::unchecked(owner),
            &crate::msg::InstantiateMsg {
                owner: owner.to_owned(),
                required_deposit: self.required_deposit.clone(),
                community_pool: self.community_pool.clone(),
                reward: self.reward.clone(),
            },
            &[],
            "Marketing Gauge Adapter",
            None,
        )
        .unwrap()
    }

    // Instantiate a cw20 and returns its address.
    fn instantiate_default_cw20(&self, app: &mut App, cw20_code_id: u64, owner: &str) -> Addr {
        let res = app
            .instantiate_contract(
                cw20_code_id,
                Addr::unchecked(owner),
                &Cw20BaseInstantiateMsg {
                    name: CW20.to_owned(),
                    symbol: CW20.to_owned(),
                    decimals: 6,
                    initial_balances: self.cw20_funds.clone(),
                    mint: Some(MinterResponse {
                        minter: owner.to_string(),
                        cap: None,
                    }),
                    marketing: None,
                },
                &[],
                CW20.to_owned(),
                None,
            )
            .unwrap();
        println!("{:#?}", res);
        res
    }

    #[track_caller]
    pub fn build(self) -> Suite {
        let mut app = App::default();
        let owner = Addr::unchecked(OWNER);

        // Store required contracts.
        let cw20_code_id = store_cw20(&mut app);
        let gauge_adapter_code_id = store_gauge_adapter(&mut app);

        // cw20 address must be "contract0" otherwise cannot use `with_cw20_deposit()`. Not very
        // elegant but do its job.
        let cw20_addr = self.instantiate_default_cw20(&mut app, cw20_code_id, OWNER);

        // Instantiate default contracts.
        let gauge_adapter_addr =
            self.instantiate_marketing_gauge_adapter(&mut app, gauge_adapter_code_id, OWNER);

        // Mint initial native token if any.
        app.init_modules(|router, _, storage| -> AnyResult<()> {
            for (addr, coin) in self.funds {
                router.bank.init_balance(storage, &addr, coin)?;
            }
            Ok(())
        })
        .unwrap();

        Suite {
            owner,
            app,
            gauge_adapter: gauge_adapter_addr,
            default_cw20: cw20_addr,
            cw20_code_id,
        }
    }
}

pub struct Suite {
    pub owner: Addr,
    pub app: App,
    pub gauge_adapter: Addr,
    pub default_cw20: Addr,
    // This is stored to instantiate other cw20 tokens in tests.
    cw20_code_id: u64,
}

impl Suite {
    // ---------------------------------------------------------------------------------------------
    // Execute
    // ---------------------------------------------------------------------------------------------
    pub fn execute_create_submission(
        &mut self,
        sender: Addr,
        name: String,
        url: String,
        address: String,
        funds: &[Coin],
    ) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            sender,
            self.gauge_adapter.clone(),
            &ExecuteMsg::CreateSubmission { name, url, address },
            funds,
        )
    }

    pub fn execute_receive_through_cw20(
        &mut self,
        sender: Addr,
        name: String,
        url: String,
        address: String,
        // amount refers to CW20 amount.
        amount: u128,
        cw20_addr: Addr,
    ) -> AnyResult<AppResponse> {
        let msg: Binary = to_json_binary(&ReceiveMsg::CreateSubmission { name, url, address })?;

        self.app.execute_contract(
            sender,
            cw20_addr,
            &Cw20BaseExecuteMsg::Send {
                contract: self.gauge_adapter.to_string(),
                amount: Uint128::from(amount),
                msg,
            },
            &[],
        )
    }

    pub fn execute_return_deposit(&mut self, sender: &str) -> AnyResult<AppResponse> {
        self.app.execute_contract(
            Addr::unchecked(sender),
            self.gauge_adapter.clone(),
            &ExecuteMsg::ReturnDeposits {},
            &[],
        )
    }

    // ---------------------------------------------------------------------------------------------
    // Queries
    // ---------------------------------------------------------------------------------------------
    pub fn query_submission(&self, address: String) -> AnyResult<SubmissionResponse> {
        Ok(self.app.wrap().query_wasm_smart(
            self.gauge_adapter.clone(),
            &AdapterQueryMsg::Submission { address },
        )?)
    }

    pub fn query_submissions(&self) -> AnyResult<Vec<SubmissionResponse>> {
        let res: AllSubmissionsResponse = self.app.wrap().query_wasm_smart(
            self.gauge_adapter.clone(),
            &AdapterQueryMsg::AllSubmissions {},
        )?;

        Ok(res.submissions)
    }

    pub fn query_all_options(&self) -> AnyResult<Vec<String>> {
        let res: AllOptionsResponse = self
            .app
            .wrap()
            .query_wasm_smart(self.gauge_adapter.clone(), &AdapterQueryMsg::AllOptions {})?;

        Ok(res.options)
    }

    pub fn query_check_option(&self, option: String) -> AnyResult<bool> {
        let res: CheckOptionResponse = self.app.wrap().query_wasm_smart(
            self.gauge_adapter.clone(),
            &AdapterQueryMsg::CheckOption { option },
        )?;

        Ok(res.valid)
    }

    // ---------------------------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------------------------

    // Instantiate a cw20 token and assign to address a specific amount.
    pub fn instantiate_token(&mut self, address: &str, token: &str, amount: u128) -> Addr {
        self.app
            .instantiate_contract(
                self.cw20_code_id,
                Addr::unchecked(address),
                &Cw20BaseInstantiateMsg {
                    name: token.to_owned(),
                    symbol: token.to_owned(),
                    decimals: 6,
                    initial_balances: vec![Cw20Coin {
                        address: address.to_owned(),
                        amount: Uint128::from(amount),
                    }],
                    mint: None,
                    marketing: None,
                },
                &[],
                token,
                None,
            )
            .unwrap()
    }

    pub fn query_cw20_balance(&self, user: &str, contract: &Addr) -> AnyResult<u128> {
        let balance: BalanceResponse = self.app.wrap().query_wasm_smart(
            contract,
            &Cw20QueryMsg::Balance {
                address: user.to_owned(),
            },
        )?;

        Ok(balance.balance.into())
    }

    pub fn query_native_balance(&self, user: &str) -> AnyResult<u128> {
        let balance = self.app.wrap().query_balance(user, NATIVE)?;

        Ok(balance.amount.into())
    }
}
