use crate::states::PoolState;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::ID as INSTRUCTION_SYSVAR_ID;
use anchor_spl::{
    token::Token,
    token_2022::Token2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use kamino_cpi::Kamino;

#[derive(Accounts)]
pub struct Rebalance<'info> {
    #[account(
        mut,
        constraint = authority.key() == crate::admin::id()
    )]
    pub authority: Signer<'info>,

    /// CHECK: pool vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub gamma_authority: UncheckedAccount<'info>,

    /// The program account of the pool in which the swap will be performed
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// The vault token account for token 0
    #[account(
        mut,
        constraint = token_vault.key() == pool_state.load()?.token_0_vault  || token_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        address = token_vault.mint
    )]
    pub token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Kamino deposit and withdraw related accounts.
    #[account(mut)]
    pub kamino_reserve: AccountInfo<'info>,

    #[account(mut)]
    pub kamino_lending_market: AccountInfo<'info>,

    #[account()]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    // This is where the collateral is deposited to.
    #[account(
        init_if_needed,
        payer = authority,
        token::mint = reserve_collateral_mint,
        token::authority = gamma_authority,
    )]
    pub gamma_pool_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = INSTRUCTION_SYSVAR_ID )]
    pub instruction_sysvar_account: AccountInfo<'info>,

    #[account(
        constraint = liquidity_token_program.key() == *token_mint.to_account_info().owner
    )]
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    pub collateral_token_program: Program<'info, Token>,

    pub kamino_program: Program<'info, Kamino>,
    pub token_program: Program<'info, Token>,
    pub token_program_2022: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn deposit_in_kamino<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Rebalance<'info>>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let kamino_deposit_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.kamino_program.to_account_info(),
        kamino_cpi::cpi::accounts::DepositReserveLiquidity {
            owner: ctx.accounts.gamma_authority.to_account_info(),
            reserve: ctx.accounts.kamino_reserve.to_account_info(),
            lending_market: ctx.accounts.kamino_lending_market.to_account_info(),
            lending_market_authority: ctx.accounts.lending_market_authority.to_account_info(),
            reserve_liquidity_mint: ctx.accounts.token_mint.to_account_info(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.to_account_info(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.to_account_info(),
            user_source_liquidity: ctx.accounts.user_destination_collateral.to_account_info(),
            user_destination_collateral: ctx.accounts.user_destination_collateral.to_account_info(),
            collateral_token_program: ctx.accounts.collateral_token_program.to_account_info(),
            liquidity_token_program: ctx.accounts.liquidity_token_program.to_account_info(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.to_account_info(),
        },
        signer_seeds,
    );
    kamino_cpi::cpi::deposit_reserve_liquidity(kamino_deposit_cpi_ctx, amount)?;
    Ok(())
}

pub fn withdraw_from_kamino<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Rebalance<'info>>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let kamino_withdraw_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.kamino_program.to_account_info(),
        kamino_cpi::cpi::accounts::RedeemReserveCollateral {
            owner: ctx.accounts.gamma_authority.to_account_info(),
            reserve: ctx.accounts.kamino_reserve.to_account_info(),
            lending_market: ctx.accounts.kamino_lending_market.to_account_info(),
            reserve_liquidity_mint: ctx.accounts.token_mint.to_account_info(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.to_account_info(),
            lending_market_authority: ctx.accounts.lending_market_authority.to_account_info(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.to_account_info(),
            user_source_collateral: ctx.accounts.user_destination_collateral.to_account_info(),
            user_destination_liquidity: ctx.accounts.user_destination_collateral.to_account_info(),
            collateral_token_program: ctx.accounts.collateral_token_program.to_account_info(),
            liquidity_token_program: ctx.accounts.liquidity_token_program.to_account_info(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.to_account_info(),
        },
        signer_seeds,
    );

    kamino_cpi::cpi::redeem_reserve_collateral(kamino_withdraw_cpi_ctx, amount)?;
    Ok(())
}

pub fn rebalance_kamino<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Rebalance<'info>>,
) -> Result<()> {
    let pool_state = ctx.accounts.pool_state.load()?;
    let is_token_0 = ctx.accounts.token_vault.key() == pool_state.token_0_vault;

    let signer_seeds: &[&[&[u8]]] = &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]];
    
    // add functions to calculate the amounts that need to be deposited and withdrawn
    // let amount_deposited =

    // deposit to kamino

    // withdraw from kamino

    Ok(())
}
