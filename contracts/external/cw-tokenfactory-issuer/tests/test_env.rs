// The code is used in tests but reported as dead code
// see https://github.com/rust-lang/rust/issues/46379
#![allow(dead_code)]

#[cfg(feature = "osmosis_tokenfactory")]
use cosmwasm_std::Uint128;
use cosmwasm_std::{Addr, Coin};

#[cfg(any(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
use cw_tokenfactory_issuer::msg::Metadata;

use cw_tokenfactory_issuer::msg::{AllowlistResponse, DenylistResponse, MigrateMsg};
use cw_tokenfactory_issuer::{
    msg::{
        AllowanceResponse, AllowancesResponse, DenomResponse, ExecuteMsg, InstantiateMsg,
        IsFrozenResponse, QueryMsg, StatusResponse,
    },
    ContractError,
};
use osmosis_test_tube::{
    osmosis_std::types::{
        cosmos::bank::v1beta1::{MsgSend, MsgSendResponse},
        cosmwasm::wasm::v1::{
            MsgExecuteContractResponse, MsgMigrateContract, MsgMigrateContractResponse,
        },
        osmosis::tokenfactory::v1beta1::QueryDenomAuthorityMetadataRequest,
    },
    Account, Bank, Module, OsmosisTestApp, Runner, RunnerError, RunnerExecuteResult,
    SigningAccount, TokenFactory, Wasm,
};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::PathBuf;
use std::rc::Rc;

pub struct TestEnv {
    pub test_accs: Vec<SigningAccount>,
    pub cw_tokenfactory_issuer: TokenfactoryIssuer,
}

impl TestEnv {
    pub fn new(instantiate_msg: InstantiateMsg, signer_index: usize) -> Result<Self, RunnerError> {
        let app = OsmosisTestApp::new();
        let test_accs_count: u64 = 4;
        let test_accs = Self::create_default_test_accs(&app, test_accs_count);

        let cw_tokenfactory_issuer =
            TokenfactoryIssuer::new(app, &instantiate_msg, &test_accs[signer_index])?;

        Ok(Self {
            test_accs,
            cw_tokenfactory_issuer,
        })
    }

    pub fn create_default_test_accs(
        app: &OsmosisTestApp,
        test_accs_count: u64,
    ) -> Vec<SigningAccount> {
        let default_initial_balance = [Coin::new(100_000_000_000, "uosmo")];

        app.init_accounts(&default_initial_balance, test_accs_count)
            .unwrap()
    }

    pub fn app(&self) -> &OsmosisTestApp {
        &self.cw_tokenfactory_issuer.app
    }

    pub fn tokenfactory(&self) -> TokenFactory<'_, OsmosisTestApp> {
        TokenFactory::new(self.app())
    }

    pub fn bank(&self) -> Bank<'_, OsmosisTestApp> {
        Bank::new(self.app())
    }

    pub fn token_admin(&self, denom: &str) -> String {
        self.tokenfactory()
            .query_denom_authority_metadata(&QueryDenomAuthorityMetadataRequest {
                denom: denom.to_string(),
            })
            .unwrap()
            .authority_metadata
            .unwrap()
            .admin
    }

    pub fn send_tokens(
        &self,
        to: String,
        coins: Vec<Coin>,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgSendResponse> {
        self.app().execute::<MsgSend, MsgSendResponse>(
            MsgSend {
                from_address: signer.address(),
                to_address: to,
                amount: coins
                    .into_iter()
                    .map(
                        |c| osmosis_test_tube::osmosis_std::types::cosmos::base::v1beta1::Coin {
                            denom: c.denom,
                            amount: c.amount.to_string(),
                        },
                    )
                    .collect(),
            },
            "/cosmos.bank.v1beta1.MsgSend",
            signer,
        )
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new(
            InstantiateMsg::NewToken {
                subdenom: "uusd".to_string(),
            },
            0,
        )
        .unwrap()
    }
}

#[derive(Debug)]
pub struct TokenfactoryIssuer {
    pub app: OsmosisTestApp,
    pub code_id: u64,
    pub contract_addr: String,
}

impl TokenfactoryIssuer {
    pub fn new(
        app: OsmosisTestApp,
        instantiate_msg: &InstantiateMsg,
        signer: &SigningAccount,
    ) -> Result<Self, RunnerError> {
        let wasm = Wasm::new(&app);
        let token_creation_fee = Coin::new(10000000, "uosmo");

        let code_id = wasm
            .store_code(&Self::get_wasm_byte_code(), None, signer)?
            .data
            .code_id;
        let contract_addr = wasm
            .instantiate(
                code_id,
                &instantiate_msg,
                Some(&signer.address()),
                Some("cw_tokenfactory_issuer"),
                &[token_creation_fee],
                signer,
            )?
            .data
            .address;

        Ok(Self {
            app,
            code_id,
            contract_addr,
        })
    }

    // executes
    pub fn execute(
        &self,
        execute_msg: &ExecuteMsg,
        funds: &[Coin],
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        let wasm = Wasm::new(&self.app);
        wasm.execute(&self.contract_addr, execute_msg, funds, signer)
    }

    pub fn update_contract_owner(
        &self,
        new_owner: &SigningAccount,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
                new_owner: new_owner.address(),
                expiry: None,
            }),
            &[],
            signer,
        )?;
        self.execute(
            &ExecuteMsg::UpdateOwnership(cw_ownable::Action::AcceptOwnership {}),
            &[],
            new_owner,
        )
    }
    pub fn update_tokenfactory_admin(
        &self,
        new_admin: &str,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::UpdateTokenFactoryAdmin {
                new_admin: new_admin.to_string(),
            },
            &[],
            signer,
        )
    }

    #[cfg(any(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
    pub fn set_denom_metadata(
        &self,
        metadata: Metadata,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(&ExecuteMsg::SetDenomMetadata { metadata }, &[], signer)
    }

    pub fn set_minter(
        &self,
        address: &str,
        allowance: u128,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::SetMinterAllowance {
                address: address.to_string(),
                allowance: allowance.into(),
            },
            &[],
            signer,
        )
    }
    pub fn mint(
        &self,
        address: &str,
        amount: u128,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::Mint {
                to_address: address.to_string(),
                amount: amount.into(),
            },
            &[],
            signer,
        )
    }

    pub fn set_burner(
        &self,
        address: &str,
        allowance: u128,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::SetBurnerAllowance {
                address: address.to_string(),
                allowance: allowance.into(),
            },
            &[],
            signer,
        )
    }

    pub fn burn(
        &self,
        address: &str,
        amount: u128,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::Burn {
                from_address: address.to_string(),
                amount: amount.into(),
            },
            &[],
            signer,
        )
    }

    #[cfg(feature = "osmosis_tokenfactory")]
    pub fn set_before_send_hook(
        &self,
        cosmwasm_address: String,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::SetBeforeSendHook { cosmwasm_address },
            &[],
            signer,
        )
    }

    #[cfg(feature = "osmosis_tokenfactory")]
    pub fn force_transfer(
        &self,
        signer: &SigningAccount,
        amount: Uint128,
        from_address: String,
        to_address: String,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::ForceTransfer {
                amount,
                from_address,
                to_address,
            },
            &[],
            signer,
        )
    }

    pub fn freeze(
        &self,
        status: bool,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(&ExecuteMsg::Freeze { status }, &[], signer)
    }

    pub fn deny(
        &self,
        address: &str,
        status: bool,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::Deny {
                address: address.to_string(),
                status,
            },
            &[],
            signer,
        )
    }

    pub fn allow(
        &self,
        address: &str,
        status: bool,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        self.execute(
            &ExecuteMsg::Allow {
                address: address.to_string(),
                status,
            },
            &[],
            signer,
        )
    }

    // queries
    pub fn query<T>(&self, query_msg: &QueryMsg) -> Result<T, RunnerError>
    where
        T: DeserializeOwned,
    {
        let wasm = Wasm::new(&self.app);
        wasm.query(&self.contract_addr, query_msg)
    }

    pub fn query_denom(&self) -> Result<DenomResponse, RunnerError> {
        self.query(&QueryMsg::Denom {})
    }

    pub fn query_is_frozen(&self) -> Result<IsFrozenResponse, RunnerError> {
        self.query(&QueryMsg::IsFrozen {})
    }

    pub fn query_is_denied(&self, address: &str) -> Result<StatusResponse, RunnerError> {
        self.query(&QueryMsg::IsDenied {
            address: address.to_string(),
        })
    }

    pub fn query_is_allowed(&self, address: &str) -> Result<StatusResponse, RunnerError> {
        self.query(&QueryMsg::IsAllowed {
            address: address.to_string(),
        })
    }

    pub fn query_owner(&self) -> Result<cw_ownable::Ownership<Addr>, RunnerError> {
        self.query(&QueryMsg::Ownership {})
    }

    pub fn query_mint_allowance(&self, address: &str) -> Result<AllowanceResponse, RunnerError> {
        self.query(&QueryMsg::MintAllowance {
            address: address.to_string(),
        })
    }

    pub fn query_mint_allowances(
        &self,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> Result<AllowancesResponse, RunnerError> {
        self.query(&QueryMsg::MintAllowances { start_after, limit })
    }

    pub fn query_burn_allowance(&self, address: &str) -> Result<AllowanceResponse, RunnerError> {
        self.query(&QueryMsg::BurnAllowance {
            address: address.to_string(),
        })
    }

    pub fn query_burn_allowances(
        &self,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> Result<AllowancesResponse, RunnerError> {
        self.query(&QueryMsg::BurnAllowances { start_after, limit })
    }

    pub fn query_allowlist(
        &self,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> Result<AllowlistResponse, RunnerError> {
        self.query(&QueryMsg::Allowlist { start_after, limit })
    }

    pub fn query_denylist(
        &self,
        start_after: Option<String>,
        limit: Option<u32>,
    ) -> Result<DenylistResponse, RunnerError> {
        self.query(&QueryMsg::Denylist { start_after, limit })
    }

    pub fn migrate(
        &self,
        testdata: &str,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgMigrateContractResponse> {
        let wasm = Wasm::new(&self.app);
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let wasm_byte_code =
            std::fs::read(manifest_path.join("tests").join("testdata").join(testdata)).unwrap();

        let code_id = wasm.store_code(&wasm_byte_code, None, signer)?.data.code_id;
        self.app.execute(
            MsgMigrateContract {
                sender: signer.address(),
                contract: self.contract_addr.clone(),
                code_id,
                msg: serde_json::to_vec(&MigrateMsg {}).unwrap(),
            },
            "/cosmwasm.wasm.v1.MsgMigrateContract",
            signer,
        )
    }

    fn get_wasm_byte_code() -> Vec<u8> {
        let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let byte_code = std::fs::read(
            manifest_path
                .join("..")
                .join("..")
                .join("..")
                .join("artifacts")
                .join("cw_tokenfactory_issuer.wasm"),
        );
        match byte_code {
            Ok(byte_code) => byte_code,
            // On arm processors, the above path is not found, so we try the following path
            Err(_) => std::fs::read(
                manifest_path
                    .join("..")
                    .join("..")
                    .join("..")
                    .join("artifacts")
                    .join("cw_tokenfactory_issuer-aarch64.wasm"),
            )
            .unwrap(),
        }
    }

    pub fn execute_error(err: ContractError) -> RunnerError {
        RunnerError::ExecuteError {
            msg: format!(
                "failed to execute message; message index: 0: {}: execute wasm contract failed",
                err
            ),
        }
    }
}

pub fn test_query_within_default_limit<QueryResult, SetStateClosure, QueryStateClosure>(
    gen_result: impl FnMut((usize, &String)) -> QueryResult,
    set_state: impl Fn(Rc<TestEnv>) -> SetStateClosure,
    query_state: impl Fn(Rc<TestEnv>) -> QueryStateClosure,
) where
    QueryResult: PartialEq + Debug + Clone,
    SetStateClosure: Fn(QueryResult),
    QueryStateClosure: Fn(Option<String>, Option<u32>) -> Vec<QueryResult>,
{
    let env = Rc::new(TestEnv::default());
    let test_accs_count = 10;
    let test_accs_with_allowance =
        TestEnv::create_default_test_accs(&env.cw_tokenfactory_issuer.app, test_accs_count);

    let mut sorted_addrs = test_accs_with_allowance
        .iter()
        .map(|acc| acc.address())
        .collect::<Vec<_>>();
    sorted_addrs.sort();

    let allowances = sorted_addrs
        .iter()
        .enumerate()
        .map(gen_result)
        .collect::<Vec<_>>();

    allowances
        .iter()
        .for_each(|allowance| set_state(env.clone())(allowance.clone()));

    let query = query_state(env);

    // let <n> be allowance for the sorted_addrs with index n

    // query from start with default limit
    // = [<0>..<10>] (since test_accs_count is 10)
    assert_eq!(query(None, None), allowances);

    // query from start with limit 1
    // = [<0>]
    assert_eq!(query(None, Some(1)), allowances[0..1]);

    // query start after sorted_addrs[1], limit 1
    // = [<2>]
    assert_eq!(
        query(Some(sorted_addrs[1].clone()), Some(1)),
        allowances[2..3]
    );

    // query start after sorted_addrs[1], limit 10
    // = [<2>..<10>] (since test_accs_count is 10)
    assert_eq!(
        query(Some(sorted_addrs[1].clone()), Some(10)),
        allowances[2..10]
    );

    // query start after sorted_addrs[9], with default limit
    // = []
    assert_eq!(query(Some(sorted_addrs[9].clone()), None), vec![]);
}

pub fn test_query_over_default_limit<QueryResult, SetStateClosure, QueryStateClosure>(
    gen_result: impl FnMut((usize, &String)) -> QueryResult,
    set_state: impl Fn(Rc<TestEnv>) -> SetStateClosure,
    query_state: impl Fn(Rc<TestEnv>) -> QueryStateClosure,
) where
    QueryResult: PartialEq + Debug + Clone,
    SetStateClosure: Fn(QueryResult),
    QueryStateClosure: Fn(Option<String>, Option<u32>) -> Vec<QueryResult>,
{
    let env = Rc::new(TestEnv::default());
    let test_accs_count = 40;
    let test_accs_with_allowance =
        TestEnv::create_default_test_accs(&env.cw_tokenfactory_issuer.app, test_accs_count);

    let mut sorted_addrs = test_accs_with_allowance
        .iter()
        .map(|acc| acc.address())
        .collect::<Vec<_>>();
    sorted_addrs.sort();

    let allowances = sorted_addrs
        .iter()
        .enumerate()
        .map(gen_result)
        .collect::<Vec<_>>();

    allowances
        .iter()
        .for_each(|allowance| set_state(env.clone())(allowance.clone()));

    let query = query_state(env);

    // let <n> be allowance for the sorted_addrs with index n

    // query from start with default limit
    // = [<0>..<10>]
    assert_eq!(query(None, None), allowances[..10]);

    // query start after sorted_addrs[4] with default limit
    // = [<5>..<15>] (<5> is after <4>, <15> is <5> + limit 10)
    assert_eq!(
        query(Some(sorted_addrs[4].clone()), None),
        allowances[5..15]
    );

    // max limit = 30
    assert_eq!(query(None, Some(40)), allowances[..30]);

    // start after nth, get n+1 .. n+1+limit (30)
    assert_eq!(
        query(Some(sorted_addrs[4].clone()), Some(40)),
        allowances[5..35]
    );
}
