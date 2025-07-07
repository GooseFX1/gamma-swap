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

    /// The mint of input token
    #[account(
        constraint = input_token_mint.key() == pool_state.load()?.token_0_mint || input_token_mint.key() == pool_state.load()?.token_1_mint,
    )]
    pub input_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of output token
    #[account(
        constraint = output_token_mint.key() == pool_state.load()?.token_0_mint || output_token_mint.key() == pool_state.load()?.token_1_mint,
        constraint = output_token_mint.key() != input_token_mint.key()
    )]
    pub output_token_mint: Box<InterfaceAccount<'info, Mint>>,
    /// The program account for the most recent oracle observation
    #[account(address = pool_state.load()?.observation_key)]
    pub observation_state: AccountLoader<'info, ObservationState>,
}
