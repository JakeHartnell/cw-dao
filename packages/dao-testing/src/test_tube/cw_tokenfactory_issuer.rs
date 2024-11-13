use cosmwasm_std::Coin;
use cw_tokenfactory_issuer::{
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    ContractError,
};
use osmosis_test_tube::{
    osmosis_std::types::cosmwasm::wasm::v1::{
        MsgExecuteContractResponse, MsgMigrateContract, MsgMigrateContractResponse,
    },
    Account, Module, OsmosisTestApp, Runner, RunnerError, RunnerExecuteResult, SigningAccount,
    Wasm,
};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::path::PathBuf;

#[derive(Debug)]
pub struct TokenfactoryIssuer<'a> {
    pub app: &'a OsmosisTestApp,
    pub code_id: u64,
    pub contract_addr: String,
}

impl<'a> TokenfactoryIssuer<'a> {
    pub fn new(
        app: &'a OsmosisTestApp,
        instantiate_msg: &InstantiateMsg,
        signer: &SigningAccount,
    ) -> Result<Self, RunnerError> {
        let wasm = Wasm::new(app);
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

    pub fn new_with_values(
        app: &'a OsmosisTestApp,
        code_id: u64,
        contract_addr: String,
    ) -> Result<Self, RunnerError> {
        Ok(Self {
            app,
            code_id,
            contract_addr,
        })
    }

    /// uploads contract and returns a code ID
    pub fn upload(app: &OsmosisTestApp, signer: &SigningAccount) -> Result<u64, RunnerError> {
        let wasm = Wasm::new(app);

        let code_id = wasm
            .store_code(&Self::get_wasm_byte_code(), None, signer)?
            .data
            .code_id;

        Ok(code_id)
    }

    // executes
    pub fn execute(
        &self,
        execute_msg: &ExecuteMsg,
        funds: &[Coin],
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgExecuteContractResponse> {
        let wasm = Wasm::new(self.app);
        wasm.execute(&self.contract_addr, execute_msg, funds, signer)
    }

    // queries
    pub fn query<T>(&self, query_msg: &QueryMsg) -> Result<T, RunnerError>
    where
        T: DeserializeOwned,
    {
        let wasm = Wasm::new(self.app);
        wasm.query(&self.contract_addr, query_msg)
    }

    pub fn migrate(
        &self,
        testdata: &str,
        signer: &SigningAccount,
    ) -> RunnerExecuteResult<MsgMigrateContractResponse> {
        let wasm = Wasm::new(self.app);
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
