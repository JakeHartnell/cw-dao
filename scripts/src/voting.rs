use cw_orch::prelude::*;
use dao_cw_orch::*;

pub struct DaoVotingSuite<Chain> {
    pub voting_cw4: DaoVotingCw4<Chain>,
    pub voting_cw20_staked: DaoVotingCw20Staked<Chain>,
    pub voting_cw721_roles: DaoVotingCw721Roles<Chain>,
    pub voting_cw721_staked: DaoVotingCw721Staked<Chain>,
    // pub voting_onft_staked: DaoVotingONftStaked<Chain>,
    pub voting_token_staked: DaoVotingTokenStaked<Chain>,
}

impl<Chain: CwEnv> DaoVotingSuite<Chain> {
    pub fn new(chain: Chain) -> DaoVotingSuite<Chain> {
        DaoVotingSuite::<Chain> {
            voting_cw4: DaoVotingCw4::new("voting_cw4", chain.clone()),
            voting_cw20_staked: DaoVotingCw20Staked::new("voting_cw20_staked", chain.clone()),
            voting_cw721_roles: DaoVotingCw721Roles::new("voting_cw721_roles", chain.clone()),
            voting_cw721_staked: DaoVotingCw721Staked::new("voting_cw721_staked", chain.clone()),
            voting_token_staked: DaoVotingTokenStaked::new("voting_token_staked", chain.clone()),
        }
    }

    pub fn upload(&self) -> Result<(), CwOrchError> {
        self.voting_cw4.upload()?;
        self.voting_cw20_staked.upload()?;
        self.voting_cw721_roles.upload()?;
        self.voting_cw721_staked.upload()?;
        self.voting_token_staked.upload()?;
        Ok(())
    }
}
