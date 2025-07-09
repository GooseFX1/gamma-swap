use crate::states::{AmmConfig, ObservationState, PoolState};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

#[derive(Accounts)]
pub struct QuoteSwap<'info> {
    /// The factory state to read protocol fees
    #[account(address = pool_state.load()?.amm_config)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// The program account of the pool in which the swap will be performed
    pub pool_state: AccountLoader<'info, PoolState>,

    /// The vault token account for input token
    ///
    /// CHECK: Unused for now. Included for forward compatibility
    pub input_vault: UncheckedAccount<'info>,

    /// The vault token account for output token
    ///
    /// CHECK: Unused for now. Included for forward compatibility
    pub output_vault: UncheckedAccount<'info>,

    /// CHECK: The mint of input token
    pub input_token_mint: UncheckedAccount<'info>,

    /// CHECK: The mint of output token
    pub output_token_mint: UncheckedAccount<'info>,

    /// The program account for the most recent oracle observation
    #[account(address = pool_state.load()?.observation_key)]
    pub observation_state: AccountLoader<'info, ObservationState>,
}
