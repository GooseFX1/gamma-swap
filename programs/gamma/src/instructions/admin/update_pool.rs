use crate::{error::GammaError, fees::FEE_RATE_DENOMINATOR_VALUE, states::PoolState};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;

#[derive(Accounts)]
#[instruction(param: u32, value: u64)]
pub struct UpdatePool<'info> {
    #[account(
        constraint = param==5|| authority.key() == crate::admin::id()
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
        3 => update_max_shared_token0(ctx, value),
        4 => update_max_shared_token1(ctx, value),
        5 => update_open_time(ctx),
        _ => Err(GammaError::InvalidInput.into()),
    }
}

fn update_open_time(ctx: Context<UpdatePool>) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    let block_timestamp = clock::Clock::get()?.unix_timestamp as u64;
    pool_state.open_time = block_timestamp;
    Ok(())
}

fn update_max_trade_fee_rate(ctx: Context<UpdatePool>, max_trade_fee_rate: u64) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.max_trade_fee_rate = max_trade_fee_rate;
    require_gt!(FEE_RATE_DENOMINATOR_VALUE, max_trade_fee_rate);
    Ok(())
}

fn update_max_shared_token0(ctx: Context<UpdatePool>, max_shared_token0: u64) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.max_shared_token0 = max_shared_token0;
    require_gt!(FEE_RATE_DENOMINATOR_VALUE, max_shared_token0);
    Ok(())
}

fn update_max_shared_token1(ctx: Context<UpdatePool>, max_shared_token1: u64) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.max_shared_token1 = max_shared_token1;
    require_gt!(FEE_RATE_DENOMINATOR_VALUE, max_shared_token1);
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
