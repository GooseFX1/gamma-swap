use crate::{
    error::GammaError,
    states::{PoolState, RewardInfo},
    utils::transfer_from_user_to_pool_vault,
    REWARD_VAULT_SEED,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};

#[derive(Accounts)]
#[instruction(start_time: u64)]
pub struct CreateRewards<'info> {
    #[account(mut)]
    pub reward_provider: Signer<'info>,

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
        mut,
        seeds = [
            crate::REWARD_INFO_SEED.as_bytes(),
            pool_state.key().as_ref(),
            start_time.to_le_bytes().as_ref(),
            reward_mint.key().as_ref(),
        ],
        bump,
    )]
    pub reward_info: Account<'info, RewardInfo>,

    #[account(
        mut,
        token::mint = reward_mint,
        token::authority = reward_provider,
    )]
    pub reward_providers_token_account: InterfaceAccount<'info, TokenAccount>,

    /// For reward to deposit into.
    #[account(
        init,
        seeds = [
            REWARD_VAULT_SEED.as_bytes(),
            reward_info.key().as_ref(),
        ],
        bump,
        payer = reward_provider,
        token::mint = reward_mint,
        token::authority = authority,
    )]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub reward_mint: Box<InterfaceAccount<'info, Mint>>,

    /// token Program
    pub token_program: Program<'info, Token>,

    /// Token program 2022
    pub token_program_2022: Program<'info, Token2022>,

    pub system_program: Program<'info, System>,
}

pub fn create_rewards(
    ctx: Context<CreateRewards>,
    start_time: u64,
    end_time: u64,
    reward_amount: u64,
) -> Result<()> {
    if start_time > end_time {
        return err!(GammaError::InvalidRewardTime);
    }

    let reward_info = &mut ctx.accounts.reward_info;
    reward_info.start_at = start_time;
    reward_info.end_rewards_at = end_time;

    reward_info.mint = ctx.accounts.reward_mint.key();
    let time_diff = end_time
        .checked_sub(start_time)
        .ok_or(GammaError::MathOverflow)?;

    reward_info.emission_per_second = reward_amount
        .checked_div(time_diff)
        .ok_or(GammaError::MathOverflow)?;

    reward_info.total_to_disburse = reward_amount;

    reward_info.rewarded_by = ctx.accounts.reward_provider.key();

    transfer_from_user_to_pool_vault(
        ctx.accounts.reward_provider.to_account_info(),
        ctx.accounts
            .reward_providers_token_account
            .to_account_info(),
        ctx.accounts.reward_vault.to_account_info(),
        ctx.accounts.reward_mint.to_account_info(),
        if ctx.accounts.reward_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        reward_amount,
        ctx.accounts.reward_mint.decimals,
    )?;

    Ok(())
}
