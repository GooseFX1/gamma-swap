use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};
use crate::{
    calculate_gamma_lp_tokens,
    instructions::deposit::{ deposit_to_gamma_pool, Deposit }, 
    states::{ MigrationEvent, PoolState, UserPoolLiquidity, USER_POOL_LIQUIDITY_SEED },
};
#[derive(Accounts)]
pub struct RaydiumCpSwapToGamma<'info> {
    pub raydium_cp_swap_program: UncheckedAccount<'info>,
    /// Owner of the liquidity provided
    #[account(mut)]
    pub owner: Signer<'info>,

    /// CHECK: pool vault authority
    #[account()]
    pub raydium_cp_swap_authority: UncheckedAccount<'info>,

    /// Pool state account
    #[account(mut)]
    pub raydium_cp_swap_pool_state: UncheckedAccount<'info>,

    /// Owner lp token account
    #[account(mut)]
    pub raydium_cp_swap_owner_lp_token: UncheckedAccount<'info>,

    // /// The owner's token account for receive token_0
    // #[account(mut)]
    // pub raydium_cp_swap_token_0_account: UncheckedAccount<'info>,

    // /// The owner's token account for receive token_1
    // #[account(mut)]
    // pub raydium_cp_swap_token_1_account: UncheckedAccount<'info>,

    /// The address that holds pool tokens for token_0
    #[account(mut)]
    pub raydium_cp_swap_token_0_vault: UncheckedAccount<'info>,

    /// The address that holds pool tokens for token_1
    #[account(mut)]
    pub raydium_cp_swap_token_1_vault: UncheckedAccount<'info>,

    /// The mint of token_0 vault
    #[account()]
    pub raydium_cp_swap_vault_0_mint: UncheckedAccount<'info>,

    /// The mint of token_1 vault
    #[account()]
    pub raydium_cp_swap_vault_1_mint: UncheckedAccount<'info>,

    /// Pool lp token mint
    #[account(mut)]
    pub raydium_cp_swap_lp_mint: UncheckedAccount<'info>,

    /// memo program
    /// CHECK:
    #[account(address = spl_memo::id())]
    pub memo_program: UncheckedAccount<'info>,

    /// Owner of the liquidity provided
    pub gamma_owner: Signer<'info>,

    /// CHECK: pool vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub gamma_authority: UncheckedAccount<'info>,

    /// Pool state the owner is depositing into
    #[account(mut)]
    pub gamma_pool_state: AccountLoader<'info, PoolState>,

    #[account(
        mut,
        seeds = [
            USER_POOL_LIQUIDITY_SEED.as_bytes(),
            gamma_pool_state.key().as_ref(),
            gamma_owner.key().as_ref(), 
        ],
        bump,
    )]
    pub gamma_user_pool_liquidity: Account<'info, UserPoolLiquidity>,

    // /// CHECK: The destination token account for receive amount_0
    // pub raydium_recipient_token_account_0: UncheckedAccount<'info>,
    /// The payer's token account to deposit token_0
    #[account(
        mut,
        token::mint = gamma_token_0_vault.mint,
        token::authority = gamma_owner
    )]
    pub gamma_token_0_account: Box<InterfaceAccount<'info, TokenAccount>>,


    // /// CHECK: The destination token account for receive amount_1
    // pub raydium_recipient_token_account_1: UncheckedAccount<'info>,
    /// The payer's token account to deposit token_1
    #[account(
        mut,
        token::mint = gamma_token_1_vault.mint,
        token::authority = gamma_owner
    )]
    pub gamma_token_1_account: Box<InterfaceAccount<'info, TokenAccount>>,
    /// Pool vault for token_0 to deposit into
    /// The address that holds pool tokens for token_0
    #[account(
        mut,
        constraint = gamma_token_0_vault.key() == gamma_pool_state.load()?.token_0_vault
    )]
    pub gamma_token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    /// Pool vault for token_1 to deposit into
    /// The address that holds pool tokens for token_1
    #[account(
        mut,
        constraint = gamma_token_1_vault.key() == gamma_pool_state.load()?.token_1_vault
    )]
    pub gamma_token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The mint of token_0 vault
    #[account(
        address = gamma_token_0_vault.mint
    )]
    pub gamma_vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of token_1 vault
    #[account(
        address = gamma_token_1_vault.mint
    )]
    pub gamma_vault_1_mint: Box<InterfaceAccount<'info, Mint>>,
    
    /// token Program
    pub token_program: Program<'info, Token>,

    /// Token program 2022
    pub token_program_2022: Program<'info, Token2022>,
}

pub fn raydium_cp_swap_to_gamma<'a, 'b, 'c, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, RaydiumCpSwapToGamma<'info>>,
    lp_token_amount_withdraw: u64,
    minimum_token_0_amount: u64,
    minimum_token_1_amount: u64,
    maximum_token_0_amount: u64,
    maximum_token_1_amount: u64,
) -> Result<()> {
    let user_token0_balance_before = ctx.accounts.gamma_token_0_account.amount;
    let user_token1_balance_before = ctx.accounts.gamma_token_1_account.amount;
    // Withdraw from Raydium CPMM
    let cpi_accounts = cpmm_cpi::cpi::accounts::Withdraw {
        owner: ctx.accounts.owner.to_account_info(),
        authority: ctx.accounts.raydium_cp_swap_authority.to_account_info(),
        pool_state: ctx.accounts.raydium_cp_swap_pool_state.to_account_info(),
        owner_lp_token: ctx.accounts.raydium_cp_swap_owner_lp_token.to_account_info(),
        token0_account: ctx.accounts.gamma_token_0_account.to_account_info(),
        token1_account: ctx.accounts.gamma_token_1_account.to_account_info(),
        token0_vault: ctx.accounts.raydium_cp_swap_token_0_vault.to_account_info(),
        token1_vault: ctx.accounts.raydium_cp_swap_token_1_vault.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        token_program2022: ctx.accounts.token_program_2022.to_account_info(),
        vault0_mint: ctx.accounts.raydium_cp_swap_vault_0_mint.to_account_info(),
        vault1_mint: ctx.accounts.raydium_cp_swap_vault_1_mint.to_account_info(),
        lp_mint: ctx.accounts.raydium_cp_swap_lp_mint.to_account_info(),
        memo_program: ctx.accounts.memo_program.to_account_info(),
    };
    let cpi_context = CpiContext::new(ctx.accounts.raydium_cp_swap_program.to_account_info(), cpi_accounts);
    cpmm_cpi::cpi::withdraw(cpi_context, lp_token_amount_withdraw, minimum_token_0_amount, minimum_token_1_amount)?;
    
    ctx.accounts.gamma_token_0_account.reload()?;
    ctx.accounts.gamma_token_1_account.reload()?;
    
    let user_token0_balance_after = ctx.accounts.gamma_token_0_account.amount;
    let user_token1_balance_after = ctx.accounts.gamma_token_1_account.amount;
    let token_0_amount_withdrawn = user_token0_balance_before.checked_sub(user_token0_balance_after).unwrap();
    let token_1_amount_withdrawn = user_token1_balance_before.checked_sub(user_token1_balance_after).unwrap();
    let pool_state = ctx.accounts.gamma_pool_state.load()?;
    let gamma_lp_tokens = calculate_gamma_lp_tokens(
        token_0_amount_withdrawn, 
        token_1_amount_withdrawn, 
        &pool_state,
        ctx.accounts.gamma_token_0_vault.amount,
        ctx.accounts.gamma_token_1_vault.amount,
    )?;
    
    // Prepare deposit accounts
    let mut deposit_accounts = Deposit {
        owner: ctx.accounts.gamma_owner.clone(),
        authority: ctx.accounts.gamma_authority.clone(),
        pool_state: ctx.accounts.gamma_pool_state.clone(),
        user_pool_liquidity: ctx.accounts.gamma_user_pool_liquidity.clone(),
        token_0_account: ctx.accounts.gamma_token_0_account.clone(),
        token_1_account: ctx.accounts.gamma_token_1_account.clone(),
        token_0_vault: ctx.accounts.gamma_token_0_vault.clone(),
        token_1_vault: ctx.accounts.gamma_token_1_vault.clone(),
        token_program: ctx.accounts.token_program.clone(),
        token_program_2022: ctx.accounts.token_program_2022.clone(),
        vault_0_mint: ctx.accounts.gamma_vault_0_mint.clone(),
        vault_1_mint: ctx.accounts.gamma_vault_1_mint.clone(),
    };

    // Deposit into Gamma pool
    deposit_to_gamma_pool(&mut deposit_accounts, gamma_lp_tokens as u64, maximum_token_0_amount, maximum_token_1_amount)?;

    // Emit event for successful migration
    emit!(MigrationEvent {
        from_pool: ctx.accounts.raydium_cp_swap_pool_state.key(),
        to_pool: ctx.accounts.gamma_pool_state.key(),
        token_0_amount_withdrawn,
        token_1_amount_withdrawn,
        lp_tokens_migrated: gamma_lp_tokens,
    });

    Ok(())
}