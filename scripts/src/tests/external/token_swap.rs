use cw_orch::{anyhow, prelude::*};
use cw_token_swap::msg::{Counterparty, InstantiateMsg, TokenInfo};
use dao_cw_orch::DaoExternalTokenSwap;

fn _setup_tokenswap_helper(
    app: DaoExternalTokenSwap<MockBech32>,
    sender: String,
    counterparty: String,
) -> anyhow::Result<()> {
    app.instantiate(
        &InstantiateMsg {
            counterparty_one: Counterparty {
                address: sender,
                promise: TokenInfo::Native {
                    denom: "juno".to_string(),
                    amount: 1_000u128.into(),
                },
            },
            counterparty_two: Counterparty {
                address: counterparty,
                promise: TokenInfo::Native {
                    denom: "juno".to_string(),
                    amount: 1_000u128.into(),
                },
            },
        },
        None,
        None,
    )?;
    Ok(())
}
