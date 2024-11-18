pub mod meteora;
pub mod orca;
pub mod raydium;

pub use meteora::*;
pub use orca::*;
pub use raydium::*;

use anchor_lang::prelude::*;
use crate::{curve::ConstantProductCurve, error::GammaError, states::PoolState};

pub fn calculate_gamma_lp_tokens(
    token_0_amount_withdrawn: u64, 
    token_1_amount_withdrawn: u64, 
    pool_state: &PoolState,
    total_vault_0_amount: u64,
    total_vault_1_amount: u64,
) -> Result<u128> {
    let (total_token_0_amount, total_token_1_amount) = pool_state.vault_amount_without_fee(
        total_vault_0_amount,
        total_vault_1_amount,
    )?;
    
    let gamma_lp_tokens_0 = ConstantProductCurve::token_0_to_lp_tokens(
        u128::from(token_0_amount_withdrawn),
        u128::from(total_token_0_amount),
        u128::from(pool_state.lp_supply),
    ).ok_or(GammaError::InvalidLpTokenAmount)?;

    let gamma_lp_tokens_1 = ConstantProductCurve::token_1_to_lp_tokens(
        u128::from(token_1_amount_withdrawn),
        u128::from(total_token_1_amount),
        u128::from(pool_state.lp_supply),
    ).ok_or(GammaError::InvalidLpTokenAmount)?;

    let gamma_lp_tokens = gamma_lp_tokens_0.min(gamma_lp_tokens_1);

    Ok(gamma_lp_tokens)
}
