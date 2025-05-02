use anchor_lang::prelude::*;
use rust_decimal::Decimal;

use crate::error::GammaError;
use rust_decimal::prelude::FromPrimitive;

#[account]
pub struct RewardInfo {
    pub pool: Pubkey,
    pub start_at: u64, // Start time for the reward UNIX timestamp.
    pub end_rewards_at: u64,
    pub mint: Pubkey,
    pub total_to_disburse: u64, // Total rewards to distribute in this unix timestamp.
    pub rewarded_by: Pubkey,    // The reward given by
    pub amount_disbursed: u64,  // Amount of rewards disbursed.
}

impl RewardInfo {
    // Additional 8 bytes, as we will add `amount_disbursed` in comming up PR to make sure we don't overflow distribution.
    pub const LEN: usize = 8 + std::mem::size_of::<RewardInfo>() + 8;

    pub fn get_time_diff(&self) -> Result<Decimal> {
        let time_diff = self
            .end_rewards_at
            .checked_sub(self.start_at)
            .ok_or(error!(GammaError::MathOverflow))?;

        Decimal::from_u64(time_diff).ok_or(error!(GammaError::MathOverflow))
    }

    pub fn get_amount_left_to_disburse(&self) -> Result<u64> {
        self.total_to_disburse
            .checked_sub(self.amount_disbursed)
            .ok_or(error!(GammaError::MathOverflow))
    }
}
