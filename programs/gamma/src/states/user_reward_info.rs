use anchor_lang::prelude::*;

use crate::error::GammaError;

use super::{GlobalRewardInfo, RewardInfo};

#[account]
pub struct UserRewardInfo {
    pub user_pool_lp_account: Pubkey,    // The user’s LP account.
    pub reward_info: Pubkey,             // The reward info account.
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
        global_rewards: &mut Account<'info, GlobalRewardInfo>,
        reward_info: &Account<'info, RewardInfo>,
    ) -> Result<()> {
        let reward_index = global_rewards
            .active_boosted_reward_info
            .iter()
            .position(|r| *r == reward_info.key());

        if reward_index.is_none() {
            return Ok(());
        }
        let reward_index = reward_index.unwrap();

        let mut last_disbursed_till = reward_info.start_at.max(self.rewards_last_calculated_at);

        for snapshot in &mut global_rewards.snapshots {
            if reward_info.end_rewards_at < snapshot.timestamp {
                break;
            }

            match reward_index {
                0 => {
                    snapshot.lp_amount_reward_0 = snapshot
                        .lp_amount_reward_0
                        .checked_add(lp_owned_by_user)
                        .ok_or(error!(GammaError::MathOverflow))?;
                }

                1 => {
                    snapshot.lp_amount_reward_1 = snapshot
                        .lp_amount_reward_1
                        .checked_add(lp_owned_by_user)
                        .ok_or(error!(GammaError::MathOverflow))?;
                }
                2 => {
                    snapshot.lp_amount_reward_2 = snapshot
                        .lp_amount_reward_2
                        .checked_add(lp_owned_by_user)
                        .ok_or(error!(GammaError::MathOverflow))?;
                }
                _ => {}
            }

            if last_disbursed_till > snapshot.timestamp {
                continue;
            }

            let duration = snapshot
                .timestamp
                .checked_sub(last_disbursed_till)
                .ok_or(GammaError::MathOverflow)?;

            self.total_rewards = self
                .total_rewards
                .checked_add(
                    reward_info
                        .emission_per_second
                        .checked_mul(duration)
                        .ok_or(GammaError::MathOverflow)?
                        .checked_mul(lp_owned_by_user)
                        .ok_or(GammaError::MathOverflow)?
                        .checked_div(current_lp_supply)
                        .ok_or(GammaError::MathOverflow)?,
                )
                .ok_or(GammaError::MathOverflow)?;

            last_disbursed_till = snapshot.timestamp;
        }

        let clock_current_time = Clock::get()?.unix_timestamp as u64;

        let duration = clock_current_time
            .checked_sub(last_disbursed_till)
            .ok_or(GammaError::MathOverflow)?;

        self.total_rewards = self
            .total_rewards
            .checked_add(
                reward_info
                    .emission_per_second
                    .checked_mul(duration)
                    .ok_or(GammaError::MathOverflow)?
                    .checked_mul(lp_owned_by_user)
                    .ok_or(GammaError::MathOverflow)?
                    .checked_div(current_lp_supply)
                    .ok_or(GammaError::MathOverflow)?,
            )
            .ok_or(GammaError::MathOverflow)?;

        self.rewards_last_calculated_at = clock_current_time;

        Ok(())
    }
}
