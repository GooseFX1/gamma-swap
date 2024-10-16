use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, Token2022},
};
use raydium_cp_swap::{
    cpi,
    program::RaydiumCpSwap,
    states::PoolState as RaydiumPoolState,
};
use crate::{curve::{CurveCalculator, RoundDirection}, error::GammaError, states::{LpChangeEvent, PoolState as GammaPoolState, PoolStatusBitIndex, UserPoolLiquidity}, utils::{get_transfer_inverse_fee, transfer_from_user_to_pool_vault}};

#[derive(Accounts)]
pub struct RaydiumCpSwapToGamma<'info> {
    pub cp_swap_program: Program<'info, RaydiumCpSwap>,
    /// Pays to mint the position
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: pool vault and lp mint authority
    #[account(
        seeds = [
            raydium_cp_swap::AUTH_SEED.as_bytes(),
        ],
        seeds::program = cp_swap_program,
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    /// Pool state account
    #[account(mut)]
    pub raydium_pool_state: AccountLoader<'info, RaydiumPoolState>,

    /// Owner lp token account
    #[account(
        mut, 
        token::authority = owner
    )]
    pub owner_lp_token: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The owner's token account for receive token_0
    #[account(
        mut,
        token::mint = token_0_vault.mint,
        token::authority = owner
    )]
    pub token_0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The owner's token account for receive token_1
    #[account(
        mut,
        token::mint = token_1_vault.mint,
        token::authority = owner
    )]
    pub token_1_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The address that holds pool tokens for token_0
    #[account(
        mut,
        constraint = token_0_vault.key() == raydium_pool_state.load()?.token_0_vault
    )]
    pub token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The address that holds pool tokens for token_1
    #[account(
        mut,
        constraint = token_1_vault.key() == raydium_pool_state.load()?.token_1_vault
    )]
    pub token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// token Program
    pub token_program: Program<'info, Token>,

    /// Token program 2022
    pub token_program_2022: Program<'info, Token2022>,

    /// The mint of token_0 vault
    #[account(address = token_0_vault.mint)]
    pub vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of token_1 vault
    #[account(address = token_1_vault.mint)]
    pub vault_1_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Pool lp token mint
    #[account(
        mut,
        address = raydium_pool_state.load()?.lp_mint
    )]
    pub lp_mint: Box<InterfaceAccount<'info, Mint>>,

    /// memo program
    /// CHECK:
    #[account(address = spl_memo::id())]
    pub memo_program: UncheckedAccount<'info>,

    /// Gamma pool state the owner is depositing into
    #[account(mut)]
    pub gamma_pool_state: AccountLoader<'info, GammaPoolState>,

    #[account(
        mut,
        seeds = [
            crate::states::USER_POOL_LIQUIDITY_SEED.as_bytes(),
            gamma_pool_state.key().as_ref(),
            owner.key().as_ref(), 
        ],
        bump,
    )]
    pub user_pool_liquidity: Account<'info, UserPoolLiquidity>,

    /// Gamma pool vault for token_0
    #[account(
        mut,
        constraint = gamma_token_0_vault.key() == gamma_pool_state.load()?.token_0_vault
    )]
    pub gamma_token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Gamma pool vault for token_1
    #[account(
        mut,
        constraint = gamma_token_1_vault.key() == gamma_pool_state.load()?.token_1_vault
    )]
    pub gamma_token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,
}

pub fn raydium_cp_swap_to_gamma(
    ctx: Context<RaydiumCpSwapToGamma>,
    lp_token_amount: u64,
    minimum_token_0_amount: u64,
    minimum_token_1_amount: u64,
    maximum_token_0_amount: u64,
    maximum_token_1_amount: u64,
) -> Result<()> {
    // First, withdraw from Raydium CP Swap
    let cpi_accounts = cpi::accounts::Withdraw {
        owner: ctx.accounts.owner.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
        pool_state: ctx.accounts.raydium_pool_state.to_account_info(),
        owner_lp_token: ctx.accounts.owner_lp_token.to_account_info(),
        token_0_account: ctx.accounts.token_0_account.to_account_info(),
        token_1_account: ctx.accounts.token_1_account.to_account_info(),
        token_0_vault: ctx.accounts.token_0_vault.to_account_info(),
        token_1_vault: ctx.accounts.token_1_vault.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        token_program_2022: ctx.accounts.token_program_2022.to_account_info(),
        vault_0_mint: ctx.accounts.vault_0_mint.to_account_info(),
        vault_1_mint: ctx.accounts.vault_1_mint.to_account_info(),
        lp_mint: ctx.accounts.lp_mint.to_account_info(),
        memo_program: ctx.accounts.memo_program.to_account_info(),
    };
    let cpi_context = CpiContext::new(ctx.accounts.cp_swap_program.to_account_info(), cpi_accounts);
    cpi::withdraw(cpi_context, lp_token_amount, minimum_token_0_amount, minimum_token_1_amount)?;

    let gamma_pool_id = ctx.accounts.gamma_pool_state.key();
    let gamma_pool_state = &mut ctx.accounts.gamma_pool_state.load_mut()?;
    if !gamma_pool_state.get_status_by_bit(PoolStatusBitIndex::Deposit) {
        return err!(GammaError::NotApproved);
    }
    let (total_token_0_amount, total_token_1_amount) = gamma_pool_state.vault_amount_without_fee(
        ctx.accounts.gamma_token_0_vault.amount,
        ctx.accounts.gamma_token_1_vault.amount,
    )?;
    let results = CurveCalculator::lp_tokens_to_trading_tokens(
        u128::from(lp_token_amount),
        u128::from(gamma_pool_state.lp_supply),
        u128::from(total_token_0_amount),
        u128::from(total_token_1_amount),
        RoundDirection::Ceiling,
    )
    .ok_or(GammaError::ZeroTradingTokens)?;

    let token_0_amount = u64::try_from(results.token_0_amount)
        .map_err(|_| GammaError::MathOverflow)?;
    let (transfer_token_0_amount, transfer_token_0_fee) = {
        let transfer_fee =
            get_transfer_inverse_fee(&ctx.accounts.vault_0_mint.to_account_info(), token_0_amount)?;
        (
            token_0_amount.checked_add(transfer_fee).ok_or(GammaError::MathOverflow)?,
            transfer_fee,
        )
    };

    let token_1_amount = u64::try_from(results.token_1_amount)
        .map_err(|_| GammaError::MathOverflow)?;
    let (transfer_token_1_amount, transfer_token_1_fee) = {
        let transfer_fee =
            get_transfer_inverse_fee(&ctx.accounts.vault_1_mint.to_account_info(), token_1_amount)?;
        (
            token_1_amount.checked_add(transfer_fee).ok_or(GammaError::MathOverflow)?,
            transfer_fee,
        )
    };

    #[cfg(feature = "enable-log")]
    msg!(
        "results.token_0_amount;{}, results.token_1_amount:{},transfer_token_0_amount:{},transfer_token_0_fee:{},
            transfer_token_1_amount:{},transfer_token_1_fee:{}",
        results.token_0_amount,
        results.token_1_amount,
        transfer_token_0_amount,
        transfer_token_0_fee,
        transfer_token_1_amount,
        transfer_token_1_fee
    );

    emit!(LpChangeEvent {
        pool_id: gamma_pool_id,
        lp_amount_before: gamma_pool_state.lp_supply,
        token_0_vault_before: total_token_0_amount,
        token_1_vault_before: total_token_1_amount,
        token_0_amount,
        token_1_amount,
        token_0_transfer_fee: transfer_token_0_fee,
        token_1_transfer_fee: transfer_token_1_fee,
        change_type: 0
    });

    if transfer_token_0_amount > maximum_token_0_amount
        || transfer_token_1_amount > maximum_token_1_amount
    {
        return Err(GammaError::ExceededSlippage.into());
    }

    transfer_from_user_to_pool_vault(
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_0_account.to_account_info(),
        ctx.accounts.token_0_vault.to_account_info(),
        ctx.accounts.vault_0_mint.to_account_info(),
        if ctx.accounts.vault_0_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        transfer_token_0_amount,
        ctx.accounts.vault_0_mint.decimals,
    )?;

    transfer_from_user_to_pool_vault(
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_1_account.to_account_info(),
        ctx.accounts.token_1_vault.to_account_info(),
        ctx.accounts.vault_1_mint.to_account_info(),
        if ctx.accounts.vault_1_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        transfer_token_1_amount,
        ctx.accounts.vault_1_mint.decimals,
    )?;

    gamma_pool_state.lp_supply = gamma_pool_state
        .lp_supply
        .checked_add(lp_token_amount)
        .ok_or(GammaError::MathOverflow)?;
    let user_pool_liquidity = &mut ctx.accounts.user_pool_liquidity;
    user_pool_liquidity.token_0_deposited = user_pool_liquidity
        .token_0_deposited
        .checked_add(u128::from(transfer_token_0_amount))
        .ok_or(GammaError::MathOverflow)?;
    user_pool_liquidity.token_1_deposited = user_pool_liquidity
        .token_1_deposited
        .checked_add(u128::from(transfer_token_1_amount))
        .ok_or(GammaError::MathOverflow)?;
    user_pool_liquidity.lp_tokens_owned = user_pool_liquidity
        .lp_tokens_owned
        .checked_add(u128::from(lp_token_amount))
        .ok_or(GammaError::MathOverflow)?;
    gamma_pool_state.recent_epoch = Clock::get()?.epoch;
    Ok(())
}
