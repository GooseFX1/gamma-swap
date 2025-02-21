use anchor_lang::prelude::*;

use crate::error::GammaError;

use super::RewardInfo;

#[account]
pub struct UserRewardInfo {
    pub total_claimed: u64,              // Total rewards claimed by the user.
    pub total_rewards: u64,              // Total rewards calculated for the user.
    pub rewards_last_calculated_at: u64, // Last time the rewards were calculated.
}

impl UserRewardInfo {
    pub fn get_total_claimable_rewards(&self) -> u64 {
        self.total_rewards.saturating_sub(self.total_claimed)
    }

    pub fn calculate_claimable_rewards<'info>(
        &mut self,
        lp_owned_by_user: u64,
        current_lp_supply: u64,
        reward_info: &Account<'info, RewardInfo>,
    ) -> Result<()> {
        let time_now = Clock::get()?.unix_timestamp as u64;
        if time_now < reward_info.start_at {
            return Ok(());
        }

        let last_disbursed_till = reward_info.start_at.max(self.rewards_last_calculated_at);

        let mut end_time = time_now;
        if reward_info.end_rewards_at < time_now {
            end_time = reward_info.end_rewards_at;
        }

        let duration = end_time
            .checked_sub(last_disbursed_till)
            .ok_or(GammaError::MathOverflow)?;

        let rewards_to_add = reward_info
            .emission_per_second
            .checked_mul(duration)
            .ok_or(GammaError::MathOverflow)?
            .checked_mul(lp_owned_by_user)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(current_lp_supply)
            .ok_or(GammaError::MathOverflow)?;

        self.total_rewards = self
            .total_rewards
            .checked_add(rewards_to_add)
            .ok_or(GammaError::MathOverflow)?;

        self.rewards_last_calculated_at = end_time;

        Ok(())
    }
}
