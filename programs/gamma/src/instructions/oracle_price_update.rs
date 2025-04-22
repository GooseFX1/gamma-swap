use crate::states::AmmConfig;
use crate::states::PoolState;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct OraclePriceUpdate<'info> {
    #[account(
        constraint = check_authority(authority.key(), &amm_config)
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        constraint = amm_config.key() == pool_state.load()?.amm_config
    )]
    pub amm_config: Account<'info, AmmConfig>,
}

fn check_authority(authority: Pubkey, amm_config: &AmmConfig) -> bool {
    return authority == amm_config.secondary_admin || authority == crate::admin::id();
}

pub fn oracle_price_update(
    ctx: Context<OraclePriceUpdate>,
    oracle_price_token_0_by_token_1: u128,
) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    pool_state.oracle_price_token_0_by_token_1 = oracle_price_token_0_by_token_1;
    let clock = Clock::get()?;
    pool_state.oracle_price_updated_at = clock.unix_timestamp as u64;
    Ok(())
}
