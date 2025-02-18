use anchor_lang::prelude::*;

use crate::error::GammaError;

use super::{GlobalRewardInfo, RewardInfo, MAX_REWARDS};

#[account]
pub struct GlobalUserLpRecentChange {
    pub rewards_calculated_upto: [u64; MAX_REWARDS],
    pub lp_snapshots: Vec<GlobalUserLpSnapshot>,
}

impl GlobalUserLpRecentChange {
    pub const MIN_SIZE: usize = 8 + (MAX_REWARDS * 8) + 4;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct GlobalUserLpSnapshot {
    pub lp_amount: u64,
    pub timestamp: u64,
}

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
        user_lp_recent_change: &mut Account<'info, GlobalUserLpRecentChange>,
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
        let time_now = Clock::get()?.unix_timestamp as u64;
        let reward_index = reward_index.unwrap();
        let lp_owned_by_user_snapshot = &mut user_lp_recent_change.lp_snapshots;
        let index_of_virtual_snapshot = lp_owned_by_user_snapshot.len();
        // add a virtual snapshot to the user's lp recent change.
        lp_owned_by_user_snapshot.push(GlobalUserLpSnapshot {
            lp_amount: lp_owned_by_user,
            timestamp: time_now,
        });

        let mut last_disbursed_till = reward_info.start_at.max(self.rewards_last_calculated_at);

        let mut has_reached_end_of_rewards = false;

        for lp_owned_by_user_snapshot in lp_owned_by_user_snapshot {
            if lp_owned_by_user_snapshot.timestamp < last_disbursed_till {
                continue;
            }

            // This works, because at the time when lp_owned_by_user
            for snapshot in &mut global_rewards.snapshots {
                if has_reached_end_of_rewards {
                    break;
                }
                if last_disbursed_till > snapshot.timestamp {
                    continue;
                }

                let mut end_time = snapshot.timestamp;
                if reward_info.end_rewards_at < snapshot.timestamp {
                    has_reached_end_of_rewards = true;
                    end_time = reward_info.end_rewards_at;
                }

                snapshot.reward_calculated_for_lp_amount[reward_index] = snapshot
                    .reward_calculated_for_lp_amount[reward_index]
                    .checked_add(lp_owned_by_user_snapshot.lp_amount)
                    .ok_or(GammaError::MathOverflow)?;

                let duration = end_time
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

                last_disbursed_till = end_time;
            }
        }

        if !has_reached_end_of_rewards {
            let end_time = std::cmp::min(time_now, reward_info.end_rewards_at);

            let duration = end_time
                .checked_sub(last_disbursed_till)
                .ok_or(GammaError::MathOverflow)?;

            global_rewards.reward_calculated_for_lp_amount[reward_index] = global_rewards
                .reward_calculated_for_lp_amount[reward_index]
                .checked_add(lp_owned_by_user)
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

            last_disbursed_till = end_time;
        }
        self.rewards_last_calculated_at = last_disbursed_till;

        user_lp_recent_change.rewards_calculated_upto[reward_index] = time_now;

        // remove the virtual snapshot.
        user_lp_recent_change
            .lp_snapshots
            .remove(index_of_virtual_snapshot);

        Ok(())
    }
}

impl GlobalUserLpRecentChange {
    pub fn remove_in_active_snapshots<'info>(
        &mut self,
        global_rewards: &mut Account<'info, GlobalRewardInfo>,
    ) -> Result<()> {
        if !global_rewards.has_any_active_rewards() {
            msg!("No active rewards");
            self.lp_snapshots.clear();
            return Ok(());
        }

        let remove_snapshots_before = self.rewards_calculated_upto.iter().min();
        if remove_snapshots_before.is_none() {
            return Ok(());
        }
        let remove_snapshots_before = *remove_snapshots_before.unwrap();

        while let Some(snapshot) = self.lp_snapshots.get(0) {
            if snapshot.timestamp < remove_snapshots_before {
                self.lp_snapshots.remove(0);
            } else {
                break;
            }
        }

        Ok(())
    }

    pub fn append_snapshot<'info>(
        &mut self,
        lp_owned_by_user: u64,
        timestamp: u64,
        global_rewards: &mut Account<'info, GlobalRewardInfo>,
    ) {
        if !global_rewards.has_any_active_rewards() {
            return;
        }

        self.lp_snapshots.push(GlobalUserLpSnapshot {
            lp_amount: lp_owned_by_user,
            timestamp,
        });
    }
}
