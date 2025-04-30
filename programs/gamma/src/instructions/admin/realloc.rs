use crate::states::{RewardInfo, UserPoolLiquidity};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ExtendUserLiquidity<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: `UserPoolLiquidity::LEN`` is strictly greater than previous,
    /// to prevent data loss
    #[account(
        mut,
        realloc = UserPoolLiquidity::LEN,
        realloc::payer = payer,
        realloc::zero = false
    )]
    pub user_pool_liquidity: Account<'info, UserPoolLiquidity>,

    pub system_program: Program<'info, System>,
}

pub fn realloc_user_liquidity(_: Context<ExtendUserLiquidity>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct ExtendRewardInfo<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: `UserPoolLiquidity::LEN`` is strictly greater than previous,
    /// to prevent data loss
    #[account(
        mut,
        realloc = RewardInfo::LEN,
        realloc::payer = payer,
        realloc::zero = false
    )]
    pub reward_info: Account<'info, RewardInfo>,

    pub system_program: Program<'info, System>,
}

pub fn realloc_reward_info(_: Context<ExtendRewardInfo>) -> Result<()> {
    Ok(())
}
