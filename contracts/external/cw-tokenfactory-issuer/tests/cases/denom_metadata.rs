use cw_tokenfactory_issuer::{msg::InstantiateMsg, ContractError};

use crate::test_env::{TestEnv, TokenfactoryIssuer};

#[cfg(any(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
#[test]
fn set_denom_metadata_by_contract_owner_should_work() {
    let subdenom = "usthb".to_string();

    // Set no metadata
    let env = TestEnv::new(InstantiateMsg::NewToken { subdenom }, 0).unwrap();
    let owner = &env.test_accs[0];

    let denom = env.cw_tokenfactory_issuer.query_denom().unwrap().denom;
    let metadata = cw_tokenfactory_issuer::msg::Metadata {
        base: denom.clone(),
        description: "Thai Baht stablecoin".to_string(),
        denom_units: vec![
            cw_tokenfactory_issuer::msg::DenomUnit {
                denom: denom.clone(),
                exponent: 0,
                aliases: vec!["sthb".to_string()],
            },
            cw_tokenfactory_issuer::msg::DenomUnit {
                denom: "sthb".to_string(),
                exponent: 6,
                aliases: vec![],
            },
        ],
        display: "sthb".to_string(),
        name: "Stable Thai Baht".to_string(),
        symbol: "STHB".to_string(),
        uri: String::default(),
        uri_hash: String::default(),
    };
    env.cw_tokenfactory_issuer
        .set_denom_metadata(metadata, owner)
        .unwrap();
}

#[cfg(any(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
#[test]
fn set_denom_metadata_by_contract_non_owner_should_fail() {
    let subdenom = "usthb".to_string();

    // Set no metadata
    let env = TestEnv::new(InstantiateMsg::NewToken { subdenom }, 0).unwrap();
    let non_owner = &env.test_accs[1];

    let denom = env.cw_tokenfactory_issuer.query_denom().unwrap().denom;
    let metadata = cw_tokenfactory_issuer::msg::Metadata {
        base: denom.clone(),
        description: "Thai Baht stablecoin".to_string(),
        denom_units: vec![
            cw_tokenfactory_issuer::msg::DenomUnit {
                denom,
                exponent: 0,
                aliases: vec!["sthb".to_string()],
            },
            cw_tokenfactory_issuer::msg::DenomUnit {
                denom: "sthb".to_string(),
                exponent: 6,
                aliases: vec![],
            },
        ],
        display: "sthb".to_string(),
        name: "Stable Thai Baht".to_string(),
        symbol: "STHB".to_string(),
        uri: String::default(),
        uri_hash: String::default(),
    };

    // Set denom metadata
    let err = env
        .cw_tokenfactory_issuer
        .set_denom_metadata(metadata, non_owner)
        .unwrap_err();

    assert_eq!(
        err,
        TokenfactoryIssuer::execute_error(ContractError::Ownership(
            cw_ownable::OwnershipError::NotOwner
        ))
    )
}

#[cfg(any(feature = "osmosis_tokenfactory", feature = "cosmwasm_tokenfactory"))]
#[test]
fn set_denom_metadata_with_base_denom_unit_should_overides_default_base_denom_unit() {
    let subdenom = "usthb".to_string();

    // Set no metadata
    let env = TestEnv::new(InstantiateMsg::NewToken { subdenom }, 0).unwrap();
    let owner = &env.test_accs[0];

    let denom = env.cw_tokenfactory_issuer.query_denom().unwrap().denom;
    let metadata = cw_tokenfactory_issuer::msg::Metadata {
        base: denom.clone(),
        description: "Thai Baht stablecoin".to_string(),
        denom_units: vec![
            cw_tokenfactory_issuer::msg::DenomUnit {
                denom: denom.clone(),
                exponent: 0,
                aliases: vec!["sthb".to_string()],
            },
            cw_tokenfactory_issuer::msg::DenomUnit {
                denom: "sthb".to_string(),
                exponent: 6,
                aliases: vec![],
            },
        ],
        display: "sthb".to_string(),
        name: "Stable Thai Baht".to_string(),
        symbol: "STHB".to_string(),
        uri: String::default(),
        uri_hash: String::default(),
    };

    // Set denom metadata
    env.cw_tokenfactory_issuer
        .set_denom_metadata(metadata.clone(), owner)
        .unwrap();
}
