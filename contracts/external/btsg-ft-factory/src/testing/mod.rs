mod app;
mod bitsong_stargate;
mod tests;

use app::BitsongApp;
use cosmwasm_std::Addr;
use cw_multi_test::Executor;
use dao_testing::contracts::{btsg_ft_factory_contract, dao_voting_token_staked_contract};

use crate::msg::InstantiateMsg;

/// Address used to stake stuff.
pub(crate) const STAKER: &str = "staker";

pub(crate) struct CommonTest {
    app: BitsongApp,
    module_id: u64,
    factory: Addr,
}

pub(crate) fn setup_test() -> CommonTest {
    let mut app = BitsongApp::new();
    let factory_id = app.store_code(btsg_ft_factory_contract());
    let module_id = app.store_code(dao_voting_token_staked_contract());

    let factory = app
        .instantiate_contract(
            factory_id,
            Addr::unchecked("anyone"),
            &InstantiateMsg {},
            &[],
            "bitsong_fantoken_factory",
            None,
        )
        .unwrap();

    CommonTest {
        app,
        module_id,
        factory,
    }
}

// Advantage to using a macro for this is that the error trace links
// to the exact line that the error occured, instead of inside of a
// function where the assertion would otherwise happen.
macro_rules! is_error {
    ($x:expr => $e:tt) => {
        assert!(format!("{:#}", $x.unwrap_err()).contains($e))
    };
}

pub(crate) use is_error;
