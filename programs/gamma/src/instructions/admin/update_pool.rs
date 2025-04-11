use crate::fees::{
    MAX_AMOUNT_SWAPPABLE_AT_ORACLE_PRICE, MAX_ORACLE_PRICE_DIFFERENCE, MAX_ORACLE_PRICE_PREMIUM,
    MAX_SHARED_WITH_KAMINO_RATE,
};
use crate::states::AmmConfig;
use crate::{error::GammaError, fees::FEE_RATE_DENOMINATOR_VALUE, states::PoolState};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock;

#[derive(Accounts)]
#[instruction(param: u32, value: u64)]
pub struct UpdatePool<'info> {
    #[account(
        constraint = check_authority(authority.key(), &amm_config, param)
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        constraint = amm_config.key() == pool_state.load()?.amm_config
    )]
    pub amm_config: Account<'info, AmmConfig>,
}

fn check_authority(authority: Pubkey, amm_config: &AmmConfig, param: u32) -> bool {
    let kamino_based_params = vec![3, 4];
    let oracle_based_swap_params = vec![6, 7, 8, 9];
    let params_update_allowed_with_secondary_admin =
        [kamino_based_params, oracle_based_swap_params].concat();

    if params_update_allowed_with_secondary_admin.contains(&param) {
        return authority == amm_config.secondary_admin || authority == crate::admin::id();
    }

    authority == crate::admin::id()
}

pub fn update_pool(ctx: Context<UpdatePool>, param: u32, value: u64) -> Result<()> {
    match param {
        0 => update_pool_status(ctx, value as u8),
        1 => update_max_trade_fee_rate(ctx, value),
        2 => update_volatility_factor(ctx, value),
        3 => update_max_shared_token0(ctx, value),
        4 => update_max_shared_token1(ctx, value),
        5 => update_open_time(ctx),
        // Oracle based swap parameters
        6 => update_acceptable_price_difference(ctx, value),
        7 => update_max_amount_swappable_at_oracle_price(ctx, value),
        8 => update_min_trade_rate_at_oracle_price(ctx, value),
        9 => update_price_premium_for_swap_at_oracle_price(ctx, value),
        _ => Err(GammaError::InvalidInput.into()),
    }
}

fn update_acceptable_price_difference(ctx: Context<UpdatePool>, value: u64) -> Result<()> {
    require_gte!(MAX_ORACLE_PRICE_DIFFERENCE, value);
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.acceptable_price_difference = value as u32;
    Ok(())
}

fn update_max_amount_swappable_at_oracle_price(ctx: Context<UpdatePool>, value: u64) -> Result<()> {
    require_gte!(MAX_AMOUNT_SWAPPABLE_AT_ORACLE_PRICE, value);
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.max_amount_swappable_at_oracle_price = value as u32;
    Ok(())
}

fn update_min_trade_rate_at_oracle_price(ctx: Context<UpdatePool>, value: u64) -> Result<()> {
    require_gte!(FEE_RATE_DENOMINATOR_VALUE, value);
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.min_trade_rate_at_oracle_price = value as u32;
    Ok(())
}

fn update_price_premium_for_swap_at_oracle_price(
    ctx: Context<UpdatePool>,
    value: u64,
) -> Result<()> {
    require_gte!(MAX_ORACLE_PRICE_PREMIUM, value);
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.price_premium_for_swap_at_oracle_price = value as u32;
    Ok(())
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
    require_gte!(MAX_SHARED_WITH_KAMINO_RATE, max_shared_token0);
    require_gt!(FEE_RATE_DENOMINATOR_VALUE, max_shared_token0);
    Ok(())
}

fn update_max_shared_token1(ctx: Context<UpdatePool>, max_shared_token1: u64) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.max_shared_token1 = max_shared_token1;
    require_gte!(MAX_SHARED_WITH_KAMINO_RATE, max_shared_token1);
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
