use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use anchor_spl::token;

use crate::migration::orca::state::{Position, TickArray, Whirlpool, RemainingAccountsInfo};

#[derive(Accounts)]
pub struct ModifyLiquidityV2<'info> {
    #[account(mut)]
    pub whirlpool: Account<'info, Whirlpool>,

    #[account(address = *token_mint_a.to_account_info().owner)]
    pub token_program_a: Interface<'info, TokenInterface>,
    #[account(address = *token_mint_b.to_account_info().owner)]
    pub token_program_b: Interface<'info, TokenInterface>,

    /// memo program
    /// CHECK:
    #[account(
        address = spl_memo::id()
    )]
    pub memo_program: UncheckedAccount<'info>,

    pub position_authority: Signer<'info>,

    #[account(mut, has_one = whirlpool)]
    pub position: Account<'info, Position>,
    #[account(
        constraint = position_token_account.mint == position.position_mint,
        constraint = position_token_account.amount == 1
    )]
    pub position_token_account: Box<Account<'info, token::TokenAccount>>,

    #[account(address = whirlpool.token_mint_a)]
    pub token_mint_a: InterfaceAccount<'info, Mint>,
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, Mint>,

    #[account(mut, constraint = token_owner_account_a.mint == whirlpool.token_mint_a)]
    pub token_owner_account_a: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, constraint = token_vault_a.key() == whirlpool.token_vault_a)]
    pub token_vault_a: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut, constraint = token_vault_b.key() == whirlpool.token_vault_b)]
    pub token_vault_b: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, has_one = whirlpool)]
    pub tick_array_lower: AccountLoader<'info, TickArray>,
    #[account(mut, has_one = whirlpool)]
    pub tick_array_upper: AccountLoader<'info, TickArray>,
    // remaining accounts
    // - accounts for transfer hook program of token_mint_a
    // - accounts for transfer hook program of token_mint_b
}


pub fn decrease_liquidity_v2<'info>(
    ctx: Context<'_, '_, '_, 'info, ModifyLiquidityV2<'info>>,
    liquidity_amount: u128,
    token_min_a: u64,
    token_min_b: u64,
    remaining_accounts_info: Option<RemainingAccountsInfo>,
) -> Result<()> { 
    // do a cpi call to the orca whirlpool program to decrease liquidity and deposit the withdrawn token into gamma pool with same token_mint_a and token_mint_b
    
    Ok(())
}
