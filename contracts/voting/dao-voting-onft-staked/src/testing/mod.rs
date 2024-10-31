mod app;
mod execute;
mod hooks;
mod omniflix_stargate;
mod queries;
mod tests;

use app::OmniflixApp;
use cosmwasm_std::Addr;
use cw_multi_test::Executor;
use cw_utils::Duration;
use dao_testing::contracts::dao_voting_onft_staked_contract;
use dao_voting::threshold::ActiveThreshold;

use crate::msg::{InstantiateMsg, OnftCollection};

use self::execute::create_onft_collection;

/// Address used as the instantiator.
pub(crate) const DAO: &str = "dao";
/// Address used to stake.
pub(crate) const STAKER: &str = "staker";

pub(crate) struct CommonTest {
    app: OmniflixApp,
    module_id: u64,
    module: Addr,
    nft: String,
}

pub(crate) fn setup_test(
    unstaking_duration: Option<Duration>,
    active_threshold: Option<ActiveThreshold>,
) -> CommonTest {
    let mut app = OmniflixApp::new();
    let module_id = app.store_code(dao_voting_onft_staked_contract());

    let nft = create_onft_collection(&mut app, "nft", DAO, DAO);
    let module = app
        .instantiate_contract(
            module_id,
            Addr::unchecked(DAO),
            &InstantiateMsg {
                onft_collection: OnftCollection::Existing {
                    id: nft.to_string(),
                },
                unstaking_duration,
                active_threshold,
            },
            &[],
            "onft_voting",
            None,
        )
        .unwrap();

    CommonTest {
        app,
        module_id,
        module,
        nft,
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
