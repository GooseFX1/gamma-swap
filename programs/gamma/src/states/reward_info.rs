use anchor_lang::prelude::*;

#[account]
pub struct RewardInfo {
    pub start_at: u64, // Start time for the reward UNIX timestamp.
    pub end_rewards_at: u64,
    pub mint: Pubkey,
    pub total_to_disburse: u64, // Total rewards to distribute in this unix timestamp.
    pub emission_per_second: u64, // Stored for easier maths in the program.
    pub rewarded_by: Pubkey,    // The reward given by
}
