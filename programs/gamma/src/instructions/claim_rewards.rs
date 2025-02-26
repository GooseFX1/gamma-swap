use crate::{
    states::{PoolState, RewardInfo, UserRewardInfo},
    utils::transfer_from_pool_vault_to_user,
    USER_REWARD_INFO_SEED,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    /// CHECK: pool vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    #[account()]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        seeds = [
            crate::REWARD_INFO_SEED.as_bytes(),
            pool_state.key().as_ref(),
            reward_info.start_at.to_le_bytes().as_ref(),
            reward_info.mint.as_ref(),
        ],
        bump,
    )]
    pub reward_info: Account<'info, RewardInfo>,

    #[account(
        mut,
        seeds = [
            crate::REWARD_VAULT_SEED.as_bytes(),
            reward_info.key().as_ref(),
        ],
        bump,
        token::mint = reward_mint,
        token::authority = authority,
    )]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = reward_mint,
        token::authority = user,
    )]
    pub user_token_account: InterfaceAccount<'info, TokenAccount>,

    // We expect the account to be created by either the cronjob or by the user.
    #[account(
        mut,
        seeds = [
            USER_REWARD_INFO_SEED.as_bytes(),
            reward_info.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
    )]
    pub user_reward_info: Account<'info, UserRewardInfo>,

    #[account(mut,
    address = reward_info.mint
    )]
    pub reward_mint: Box<InterfaceAccount<'info, Mint>>,

    /// token Program
    pub token_program: Program<'info, Token>,

    /// Token program 2022
    pub token_program_2022: Program<'info, Token2022>,

    pub system_program: Program<'info, System>,
}

pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    let user_reward_info = &mut ctx.accounts.user_reward_info;
    let total_claimable_rewards = user_reward_info.get_total_claimable_rewards();
    if total_claimable_rewards == 0 {
        return Ok(());
    }

    let pool_state = &mut ctx.accounts.pool_state.load()?;

    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.reward_vault.to_account_info(),
        ctx.accounts.user_token_account.to_account_info(),
        ctx.accounts.reward_mint.to_account_info(),
        if ctx.accounts.reward_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        total_claimable_rewards,
        ctx.accounts.reward_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
    )?;

    user_reward_info.total_claimed = user_reward_info
        .total_claimed
        .checked_add(total_claimable_rewards)
        .unwrap();

    Ok(())
}
