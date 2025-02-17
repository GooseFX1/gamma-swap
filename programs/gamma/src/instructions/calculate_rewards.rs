use crate::{
    states::{
        GlobalRewardInfo, GlobalUserLpRecentChange, PoolState, RewardInfo, UserPoolLiquidity,
        UserRewardInfo, USER_POOL_LIQUIDITY_SEED,
    },
    USER_REWARD_INFO_SEED,
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CalculateRewards<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account()]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        mut,
        seeds = [
            crate::GLOBAL_REWARD_INFO_SEED.as_bytes(),
            pool_state.key().as_ref(),
        ],
        bump,
    )]
    pub global_reward_info: Account<'info, GlobalRewardInfo>,

    #[account(
        mut,
        seeds = [
            crate::GLOBAL_USER_LP_RECENT_CHANGE_SEED.as_bytes(),
            pool_state.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
    )]
    pub global_user_lp_recent_change: Account<'info, GlobalUserLpRecentChange>,

    #[account(
        seeds = [
            pool_state.key().as_ref(),
            reward_info.start_at.to_le_bytes().as_ref(),
            reward_info.mint.as_ref(),
            crate::REWARD_INFO_SEED.as_bytes(),
        ],
        bump,
    )]
    pub reward_info: Account<'info, RewardInfo>,

    #[account(
        init_if_needed,
        space = 8 + std::mem::size_of::<UserRewardInfo>(),
        payer = user,
        seeds = [
            reward_info.key().as_ref(),
            user.key().as_ref(),
            USER_REWARD_INFO_SEED.as_bytes(),
            ],
            bump,
        )]
    pub user_reward_info: Account<'info, UserRewardInfo>,

    /// User pool liquidity account
    #[account(
        seeds = [
            USER_POOL_LIQUIDITY_SEED.as_bytes(),
            pool_state.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
    )]
    pub user_pool_liquidity: Account<'info, UserPoolLiquidity>,

    pub system_program: Program<'info, System>,
}

pub fn calculate_rewards(ctx: Context<CalculateRewards>) -> Result<()> {
    let pool_state = &mut ctx.accounts.pool_state.load()?;
    let current_time = Clock::get()?.unix_timestamp as u64;
    if ctx.accounts.user_reward_info.rewards_last_calculated_at >= current_time {
        return Ok(());
    }

    let user_reward_info = &mut ctx.accounts.user_reward_info;
    user_reward_info.calculate_claimable_rewards(
        ctx.accounts.user_pool_liquidity.lp_tokens_owned as u64,
        pool_state.lp_supply as u64,
        &mut ctx.accounts.global_user_lp_recent_change,
        &mut ctx.accounts.global_reward_info,
        &ctx.accounts.reward_info,
    )?;

    ctx.accounts
        .global_reward_info
        .remove_inactive_rewards(&ctx.accounts.reward_info, current_time);

    ctx.accounts
        .global_reward_info
        .remove_all_inactive_snapshots();

    ctx.accounts
        .global_user_lp_recent_change
        .remove_in_active_snapshots(&mut ctx.accounts.global_reward_info)?;

    Ok(())
}
