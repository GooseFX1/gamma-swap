use crate::states::PoolState;
use crate::states::{
    GlobalRewardInfo, GlobalUserLpRecentChange, UserPoolLiquidity, USER_POOL_LIQUIDITY_SEED,
};
use anchor_lang::prelude::*;

// Instruction to create missing accounts required for rewards feature.
#[derive(Accounts)]
pub struct Migration<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,

    /// The program account of the pool in which the swap will be performed
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// Check: NO need to check the account, we only want to check if we have a corresponding user pool liquidity account
    pub owner: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            USER_POOL_LIQUIDITY_SEED.as_bytes(),
            pool_state.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump,
    )]
    pub user_pool_liquidity: Account<'info, UserPoolLiquidity>,

    #[account(
        init,
        payer = signer,
        space = GlobalUserLpRecentChange::MIN_SIZE,
        seeds = [
            crate::GLOBAL_USER_LP_RECENT_CHANGE_SEED.as_bytes(),
            pool_state.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump,
    )]
    pub global_user_lp_recent_change: Account<'info, GlobalUserLpRecentChange>,

    #[account(
        init_if_needed,
        space = GlobalRewardInfo::MIN_SIZE,
        payer = signer,
        seeds = [
            crate::GLOBAL_REWARD_INFO_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
            bump,
        )]
    pub global_reward_info: Account<'info, GlobalRewardInfo>,

    pub system_program: Program<'info, System>,
}

pub fn migration(ctx: Context<Migration>) -> Result<()> {
    Ok(())
}
