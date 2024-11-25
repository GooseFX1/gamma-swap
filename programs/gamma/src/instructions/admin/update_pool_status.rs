use anchor_lang::prelude::*;

use crate::{error::GammaError, states::PoolState};

#[derive(Accounts)]
pub struct UpdatePool<'info> {
    #[account(
        address = crate::admin::id()
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,
}

pub fn update_pool(ctx: Context<UpdatePool>, param: u32, value: u64) -> Result<()> {
    match param {
        0 => update_pool_status(ctx, value as u8),
        1 => update_max_trade_fee_rate(ctx, value),
        2 => update_volatility_factor(ctx, value),
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
