use cw_orch::prelude::*;
use dao_cw_orch::*;

pub struct DaoStakingSuite<Chain> {
    pub cw20_stake: DaoStakingCw20<Chain>,
    pub exteral_rewards: DaoStakingCw20ExternalRewards<Chain>,
    pub rewards_distributor: DaoStakingCw20RewardDistributor<Chain>,
}

impl<Chain: CwEnv> DaoStakingSuite<Chain> {
    pub fn new(chain: Chain) -> DaoStakingSuite<Chain> {
        DaoStakingSuite::<Chain> {
            cw20_stake: DaoStakingCw20::new("cw20_stake", chain.clone()),
            exteral_rewards: DaoStakingCw20ExternalRewards::new(
                "cw20_external_rewards",
                chain.clone(),
            ),
            rewards_distributor: DaoStakingCw20RewardDistributor::new(
                "cw20_reward_distributor",
                chain.clone(),
            ), 
        }
    }

    pub fn upload(&self) -> Result<(), CwOrchError> {
        self.cw20_stake.upload()?;
        self.exteral_rewards.upload()?;
        self.rewards_distributor.upload()?;
        Ok(())
    }
}
