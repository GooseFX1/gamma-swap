use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

use crate::{error::GammaError, states::PoolState};

#[derive(Accounts)]
#[instruction(param: u32, value: u64)]
pub struct UpdatePool<'info> {
    #[account(
        constraint = param == 10 || authority.key() == crate::admin::id()
    )]
    pub authority: Signer<'info>,

    /// The vault token account for input token
    #[account(
        constraint = token_0_vault.key() == pool_state.load()?.token_0_vault
    )]
    pub token_0_vault: Account<'info, TokenAccount>,

    /// The vault token account for output token
    #[account(
        constraint = token_1_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub token_1_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,
}


pub fn vault_amount_without_fee(pool_state: &PoolState, vault_0: u64, vault_1: u64) -> (u64, u64) {
     (
         vault_0
             .checked_sub(pool_state.protocol_fees_token_0 + pool_state.fund_fees_token_0)
             .unwrap(),
         vault_1
             .checked_sub(pool_state.protocol_fees_token_1 + pool_state.fund_fees_token_1)
             .unwrap(),
     )
 }

pub fn update_pool(ctx: Context<UpdatePool>, param: u32, value: u64) -> Result<()> {
    match param {
        0 => update_pool_status(ctx, value as u8),
        1 => update_max_trade_fee_rate(ctx, value),
        2 => update_volatility_factor(ctx, value),
        10 => {
            // this is temporary change to make things correct for current existing pools.
            let mut pool_state = ctx.accounts.pool_state.load_mut()?;
            let (vault_0_amount, vault_1_amount)= vault_amount_without_fee(&pool_state, ctx.accounts.token_0_vault.amount, ctx.accounts.token_1_vault.amount);
            pool_state.token_0_vault_amount = vault_0_amount;
            pool_state.token_1_vault_amount = vault_1_amount;
            Ok(())
        }
        _ => Err(GammaError::InvalidInput.into()),
    }
}

fn update_max_trade_fee_rate(ctx: Context<UpdatePool>, max_trade_fee_rate: u64) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.max_trade_fee_rate = max_trade_fee_rate;
    Ok(())
}

fn update_volatility_factor(ctx: Context<UpdatePool>, volatility_factor: u64) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.volatility_factor = volatility_factor;
    Ok(())
}

fn update_pool_status(ctx: Context<UpdatePool>, status: u8) -> Result<()> {
    require_gte!(255, status);
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.set_status(status);
    pool_state.recent_epoch = Clock::get()?.epoch;
    Ok(())
}
