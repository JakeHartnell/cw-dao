use std::collections::HashSet;

use cw_orch::{
    contract::interface_traits::Uploadable,
    environment::{ChainInfo, ChainKind, NetworkInfo},
    mock::Mock,
};

use crate::{
    DaoDaoCore, DaoPreProposeApprovalSingle, DaoPreProposeApprover, DaoPreProposeMultiple,
    DaoPreProposeSingle, DaoProposalCondorcet, DaoProposalHookCounter, DaoProposalMultiple,
    DaoProposalSingle, DaoProposalSudo, DaoStakingCw20, DaoStakingCw20ExternalRewards,
    DaoStakingCw20RewardDistributor, DaoTestCustomFactory, DaoVotingCw20Balance,
    DaoVotingCw20Staked, DaoVotingCw4, DaoVotingCw721Roles, DaoVotingCw721Staked,
    DaoVotingTokenStaked,
};

pub const DUMMY_CHAIN_INFO: ChainInfo = ChainInfo {
    chain_id: "mock-1",
    gas_denom: "none",
    gas_price: 0.0,
    grpc_urls: &[],
    lcd_url: None,
    fcd_url: None,
    network_info: NetworkInfo {
        chain_name: "mock",
        pub_address_prefix: "mock",
        coin_type: 118,
    },
    kind: ChainKind::Local,
};

#[test]
fn test_all_wasms_different() {
    let all_paths = vec![
        // CORE
        DaoDaoCore::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        // PRE-PROPOSE
        DaoPreProposeApprovalSingle::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoPreProposeApprover::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoPreProposeMultiple::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoPreProposeSingle::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        // PROPOSAL
        DaoProposalCondorcet::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoProposalMultiple::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoProposalSingle::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        // Stake
        DaoStakingCw20::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoStakingCw20ExternalRewards::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoStakingCw20RewardDistributor::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        // Voting
        DaoVotingCw4::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoVotingCw20Staked::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoVotingCw721Staked::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoVotingCw721Roles::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoVotingTokenStaked::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        // Test
        DaoProposalHookCounter::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoProposalSudo::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoTestCustomFactory::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
        DaoVotingCw20Balance::<Mock>::wasm(&DUMMY_CHAIN_INFO.into()),
    ];
    let all_paths: Vec<_> = all_paths
        .into_iter()
        .map(|path| path.path().as_os_str().to_string_lossy().to_string())
        .collect();

    let mut uniq = HashSet::new();
    assert!(all_paths.into_iter().all(move |x| uniq.insert(x)));
}
