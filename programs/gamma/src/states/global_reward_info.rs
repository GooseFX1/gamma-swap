use crate::error::GammaError;
use anchor_lang::prelude::*;

use super::RewardInfo;

pub const MAX_REWARDS: usize = 3;

#[account]
pub struct GlobalRewardInfo {
    // This contains the 3 active boosted rewards, i.e. all rewards that are not fully distributed
    // And the current time maybe exceeds the end time of the last boosted reward
    // There is never a proper endtime of the rewards we can even have active boosted rewards if they are not fully distributed yet.
    // Any reward that is not started yet is also consider active.
    pub active_boosted_reward_info: [Pubkey; MAX_REWARDS],

    // This is compared with lp_supply, This value is copied to the snapshot, if the lp_supply changes.
    // This value once it is equal to lp_supply, we can safely remove snapshot.
    pub reward_calculated_for_lp_amount: [u64; MAX_REWARDS],

    pub start_times: [Option<u64>; MAX_REWARDS],

    pub snapshots: Vec<Snapshot>,
}

impl GlobalRewardInfo {
    pub const MIN_SIZE: usize = 8 + (MAX_REWARDS * 32) + (MAX_REWARDS * (1 + 8)) + 4;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Snapshot {
    // These is to track that the amount was calculated for the boosted rewards
    // at the time of the snapshot for the lp amount
    // If lp amount_reward[0] is equal to total_lp_amount, then the reward has been fully distributed
    // and we can remove the snapshot from the queue
    pub reward_calculated_for_lp_amount: [u64; MAX_REWARDS],
    pub total_lp_amount: u64,
    pub timestamp: u64,
}

impl GlobalRewardInfo {
    pub fn add_new_active_reward(&mut self, reward_info: Pubkey, start_time: u64) -> Result<()> {
        for i in 0..MAX_REWARDS {
            if self.active_boosted_reward_info[i] == Pubkey::default() {
                self.active_boosted_reward_info[i] = reward_info;
                self.start_times[i] = Some(start_time);
                self.reward_calculated_for_lp_amount[i] = 0;
                return Ok(());
            }
        }
        return err!(GammaError::MaxRewardsReached);
    }

    pub fn has_any_active_rewards(&self) -> bool {
        for i in 0..MAX_REWARDS {
            if self.active_boosted_reward_info[i] != Pubkey::default() {
                return true;
            }
        }
        return false;
    }

    pub fn append_snapshot(&mut self, total_lp_amount: u64, timestamp: u64) {
        if !self.has_any_active_rewards() {
            return;
        }

        self.snapshots.push(Snapshot {
            total_lp_amount,
            timestamp,
            reward_calculated_for_lp_amount: self.reward_calculated_for_lp_amount,
        });
    }

    pub fn remove_inactive_rewards(
        &mut self,
        reward_info: &Account<RewardInfo>,
        current_time: u64,
    ) {
        for i in 0..MAX_REWARDS {
            if self.active_boosted_reward_info[i] == reward_info.key()
                && !reward_info.is_active(current_time)
            {
                msg!(
                    "Removing reward info as it is inactive and reward info is {}",
                    reward_info.key()
                );
                self.active_boosted_reward_info[i] = Pubkey::default();
                self.start_times[i] = None;
                break;
            }
        }
    }

    pub fn remove_all_inactive_snapshots(&mut self) {
        let is_reward_one_initialized = self.active_boosted_reward_info[0] != Pubkey::default();
        let is_reward_two_initialized = self.active_boosted_reward_info[1] != Pubkey::default();
        let is_reward_three_initialized = self.active_boosted_reward_info[2] != Pubkey::default();

        if !is_reward_one_initialized && !is_reward_two_initialized && !is_reward_three_initialized
        {
            msg!("No active rewards, clearing snapshots");
            self.snapshots.clear();
            return;
        }
        let min_start_time: u64 = self
            .start_times
            .iter()
            .filter(|x| x.is_some())
            .fold(u64::MAX, |a, b| a.min(b.unwrap()));
        if min_start_time == u64::MAX {
            msg!("No active rewards, clearing snapshots");
            self.snapshots.clear();
            return;
        }

        while let Some(snapshot) = self.snapshots.get(0) {
            let is_before_min_start_time = snapshot.timestamp < min_start_time;
            if is_before_min_start_time {
                self.snapshots.remove(0);
                continue;
            }

            let is_reward_one_fully_distributed_until_this_snapshot =
                snapshot.total_lp_amount == snapshot.reward_calculated_for_lp_amount[0];
            let is_reward_two_fully_distributed_until_this_snapshot =
                snapshot.total_lp_amount == snapshot.reward_calculated_for_lp_amount[1];
            let is_reward_three_fully_distributed_until_this_snapshot =
                snapshot.total_lp_amount == snapshot.reward_calculated_for_lp_amount[2];

            let snapshot_is_required_for_reward_one =
                is_reward_one_initialized && !is_reward_one_fully_distributed_until_this_snapshot;
            let snapshot_is_required_for_reward_two =
                is_reward_two_initialized && !is_reward_two_fully_distributed_until_this_snapshot;
            let snapshot_is_required_for_reward_three = is_reward_three_initialized
                && !is_reward_three_fully_distributed_until_this_snapshot;

            if snapshot_is_required_for_reward_one
                || snapshot_is_required_for_reward_two
                || snapshot_is_required_for_reward_three
            {
                break;
            }

            self.snapshots.remove(0);
        }
    }
}
