use anchor_lang::prelude::*;
use rust_decimal::Decimal;

use crate::{error::GammaError, LOCK_LP_AMOUNT};
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};

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

        let end_time = time_now.min(reward_info.end_rewards_at);

        let duration = end_time
            .checked_sub(last_disbursed_till)
            .ok_or(GammaError::MathOverflow)?;

        let total_to_disburse =
            Decimal::from_u64(reward_info.total_to_disburse).ok_or(GammaError::MathOverflow)?;

        let max_duration = reward_info.get_time_diff()?;
        let duration_decimal = Decimal::from_u64(duration).ok_or(GammaError::MathOverflow)?;
        let lp_owned_by_user_decimal =
            Decimal::from_u64(lp_owned_by_user).ok_or(GammaError::MathOverflow)?;

        let current_lp_supply_decimal = Decimal::from_u64(current_lp_supply)
            .ok_or(GammaError::MathOverflow)?
            // The locked liquidity is not eligible for rewards
            .checked_sub(LOCK_LP_AMOUNT.into())
            .ok_or(GammaError::MathOverflow)?;

        let rewards_to_add = total_to_disburse
            .checked_mul(duration_decimal)
            .ok_or(GammaError::MathOverflow)?
            .checked_mul(lp_owned_by_user_decimal)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(current_lp_supply_decimal)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(max_duration)
            .ok_or(GammaError::MathOverflow)?;

        self.total_rewards = self
            .total_rewards
            .checked_add(rewards_to_add.to_u64().ok_or(GammaError::MathOverflow)?)
            .ok_or(GammaError::MathOverflow)?;

        self.rewards_last_calculated_at = end_time;

        Ok(())
    }
}
