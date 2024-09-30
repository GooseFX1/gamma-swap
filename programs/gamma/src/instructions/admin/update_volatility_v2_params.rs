use anchor_lang::prelude::*;

use crate::states::PoolState;

#[derive(Accounts)]
pub struct UpdateVolatilityV2Params<'info> {
    #[account(
        address = crate::admin::id()
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,
}

pub fn update_volatility_v2_params(
    ctx: Context<UpdateVolatilityV2Params>,
    base_fee: u64,
    max_fee: u64,
    volatility_factor: u64,
    imbalance_factor: u64,
) -> Result<()> {
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    pool_state.volatility_v2_base_fee = base_fee;
    pool_state.volatility_v2_max_fee = max_fee;
    pool_state.volatility_v2_volatility_factor = volatility_factor;
    pool_state.volatility_v2_imbalance_factor = imbalance_factor;
    Ok(())
}