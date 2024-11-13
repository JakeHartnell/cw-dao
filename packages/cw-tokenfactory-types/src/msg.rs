#[cfg(feature = "osmosis_tokenfactory")]
mod tokenfactory_msg {
    use crate::osmosis::{
        MsgBurn, MsgChangeAdmin, MsgCreateDenom, MsgForceTransfer, MsgMint, MsgSetBeforeSendHook,
        MsgSetDenomMetadata,
    };
    use dao_interface::token::Metadata;
    use osmosis_std::types::cosmos::{
        bank::v1beta1::{DenomUnit as OsmosisDenomUnit, Metadata as OsmosisMetadata},
        base::v1beta1::Coin,
    };

    pub use crate::osmosis::MsgCreateDenomResponse;

    pub fn msg_create_denom(sender: String, subdenom: String) -> MsgCreateDenom {
        MsgCreateDenom { sender, subdenom }
    }

    pub fn msg_mint(sender: String, amount: u128, denom: String) -> MsgMint {
        MsgMint {
            sender: sender.clone(),
            amount: Some(Coin {
                amount: amount.to_string(),
                denom,
            }),
            mint_to_address: sender,
        }
    }

    pub fn msg_burn(
        sender: String,
        amount: u128,
        denom: String,
        burn_from_address: String,
    ) -> MsgBurn {
        MsgBurn {
            sender,
            amount: Some(Coin {
                amount: amount.to_string(),
                denom,
            }),
            burn_from_address,
        }
    }

    pub fn msg_set_denom_metadata(sender: String, metadata: Metadata) -> MsgSetDenomMetadata {
        MsgSetDenomMetadata {
            sender,
            metadata: Some(OsmosisMetadata {
                description: metadata.description,
                denom_units: metadata
                    .denom_units
                    .into_iter()
                    .map(|denom_unit| OsmosisDenomUnit {
                        denom: denom_unit.denom,
                        exponent: denom_unit.exponent,
                        aliases: denom_unit.aliases,
                    })
                    .collect(),
                base: metadata.base,
                display: metadata.display,
                name: metadata.name,
                symbol: metadata.symbol,
                uri: metadata.uri,
                uri_hash: metadata.uri_hash,
            }),
        }
    }

    pub fn msg_change_admin(sender: String, denom: String, new_admin: String) -> MsgChangeAdmin {
        MsgChangeAdmin {
            sender,
            denom,
            new_admin,
        }
    }

    pub fn msg_force_transfer(
        sender: String,
        amount: u128,
        denom: String,
        transfer_from_address: String,
        transfer_to_address: String,
    ) -> MsgForceTransfer {
        MsgForceTransfer {
            sender,
            amount: Some(Coin {
                amount: amount.to_string(),
                denom,
            }),
            transfer_from_address,
            transfer_to_address,
        }
    }

    pub fn msg_set_before_send_hook(
        sender: String,
        denom: String,
        cosmwasm_address: String,
    ) -> MsgSetBeforeSendHook {
        MsgSetBeforeSendHook {
            sender,
            denom,
            cosmwasm_address,
        }
    }
}

#[cfg(feature = "cosmwasm_tokenfactory")]
mod tokenfactory_msg {
    use crate::cosmos::{Coin, DenomUnit as CosmwasmDenomUnit, Metadata as CosmwasmMetadata};
    use crate::cosmwasm::{MsgBurn, MsgChangeAdmin, MsgCreateDenom, MsgMint, MsgSetDenomMetadata};
    use dao_interface::token::Metadata;

    pub use crate::cosmwasm::MsgCreateDenomResponse;

    pub fn msg_create_denom(sender: String, subdenom: String) -> MsgCreateDenom {
        MsgCreateDenom { sender, subdenom }
    }

    pub fn msg_mint(sender: String, amount: u128, denom: String) -> MsgMint {
        MsgMint {
            sender,
            amount: Some(Coin {
                amount: amount.to_string(),
                denom,
            }),
        }
    }

    pub fn msg_burn(
        sender: String,
        amount: u128,
        denom: String,
        _burn_from_address: String,
    ) -> MsgBurn {
        MsgBurn {
            sender,
            amount: Some(Coin {
                amount: amount.to_string(),
                denom,
            }),
        }
    }

    pub fn msg_set_denom_metadata(sender: String, metadata: Metadata) -> MsgSetDenomMetadata {
        MsgSetDenomMetadata {
            sender,
            metadata: Some(CosmwasmMetadata {
                description: metadata.description,
                denom_units: metadata
                    .denom_units
                    .into_iter()
                    .map(|denom_unit| CosmwasmDenomUnit {
                        denom: denom_unit.denom,
                        exponent: denom_unit.exponent,
                        aliases: denom_unit.aliases,
                    })
                    .collect(),
                base: metadata.base,
                display: metadata.display,
                name: metadata.name,
                symbol: metadata.symbol,
            }),
        }
    }

    pub fn msg_change_admin(sender: String, denom: String, new_admin: String) -> MsgChangeAdmin {
        MsgChangeAdmin {
            sender,
            denom,
            new_admin,
        }
    }
}

#[cfg(feature = "kujira_tokenfactory")]
mod tokenfactory_msg {
    use crate::{
        cosmos::Coin,
        kujira::{MsgBurn, MsgChangeAdmin, MsgCreateDenom, MsgMint},
    };

    pub use crate::cosmwasm::MsgCreateDenomResponse;

    pub fn msg_create_denom(sender: String, subdenom: String) -> MsgCreateDenom {
        MsgCreateDenom {
            sender,
            nonce: subdenom,
        }
    }

    pub fn msg_mint(sender: String, amount: u128, denom: String) -> MsgMint {
        MsgMint {
            sender: sender.clone(),
            amount: Some(Coin {
                amount: amount.to_string(),
                denom,
            }),
            // other tokenfactories only support minting to the sender, so force
            // this behavior to be the same
            recipient: sender,
        }
    }

    pub fn msg_burn(
        sender: String,
        amount: u128,
        denom: String,
        _burn_from_address: String,
    ) -> MsgBurn {
        MsgBurn {
            sender,
            amount: Some(Coin {
                amount: amount.to_string(),
                denom,
            }),
        }
    }

    pub fn msg_change_admin(sender: String, denom: String, new_admin: String) -> MsgChangeAdmin {
        MsgChangeAdmin {
            sender,
            denom,
            new_admin,
        }
    }
}

// re-export chosen tokenfactory standard
#[cfg(any(
    feature = "osmosis_tokenfactory",
    feature = "cosmwasm_tokenfactory",
    feature = "kujira_tokenfactory"
))]
pub use tokenfactory_msg::*;

// require one tokenfactory standard to be chosen
#[cfg(not(any(
    feature = "osmosis_tokenfactory",
    feature = "cosmwasm_tokenfactory",
    feature = "kujira_tokenfactory"
)))]
compile_error!(
    "feature \"osmosis_tokenfactory\", \"cosmwasm_tokenfactory\", or \"kujira_tokenfactory\" must be enabled"
);

// prevent more than one tokenfactory standard from being chosen

#[cfg(all(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
compile_error!("feature \"osmosis_tokenfactory\" and feature \"cosmwasm_tokenfactory\" cannot be enabled at the same time");

#[cfg(all(feature = "osmosis_tokenfactory", feature = "kujira_tokenfactory"))]
compile_error!("feature \"osmosis_tokenfactory\" and feature \"kujira_tokenfactory\" cannot be enabled at the same time");

#[cfg(all(feature = "cosmwasm_tokenfactory", feature = "kujira_tokenfactory"))]
compile_error!("feature \"cosmwasm_tokenfactory\" and feature \"kujira_tokenfactory\" cannot be enabled at the same time");
