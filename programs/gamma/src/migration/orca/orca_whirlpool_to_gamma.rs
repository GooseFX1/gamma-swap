use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{ Mint, Token2022, TokenAccount},
};
use crate::{
    calculate_gamma_lp_tokens,
    instructions::deposit::{ deposit_to_gamma_pool, Deposit }, 
    states::{ MigrationEvent, PoolState, UserPoolLiquidity, USER_POOL_LIQUIDITY_SEED },
};

#[derive(Accounts)]
pub struct OrcaWhirlpoolToGamma<'info> {
    /// CHECK: Whirlpool program
    #[account(address = whirlpool_cpi::ID)]
    pub whirlpool_program: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Whirlpool
    pub whirlpool: UncheckedAccount<'info>,

    #[account(address = *gamma_vault_0_mint.to_account_info().owner)]
    /// CHECK: Token program of mint A
    pub token_program_a: UncheckedAccount<'info>,
    #[account(address = *gamma_vault_1_mint.to_account_info().owner)]
    /// CHECK: Token program of mint B
    pub token_program_b: UncheckedAccount<'info>,

    /// CHECK: Memo program
    #[account(
        address = spl_memo::id()
    )]
    pub memo_program: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Position
    pub whirlpool_position: UncheckedAccount<'info>,
    #[account()]
    /// CHECK: Position token account
    pub whirlpool_position_token_account: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Token vault A
    pub whirlpool_token_vault_a: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Token vault B
    pub whirlpool_token_vault_b: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Tick array lower
    pub whirlpool_tick_array_lower: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Tick array upper
    pub whirlpool_tick_array_upper: UncheckedAccount<'info>,

    // /// Position authority(User)
    // pub whirlpool_position_authority: Signer<'info>,
    /// The owner LP Position in Gamma pool
    pub gamma_owner: Signer<'info>,
    
    /// CHECK: pool vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub gamma_authority: UncheckedAccount<'info>,

    /// Gamma Pool state the owner is depositing into
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

    // /// CHECK: Token owner account A
    // pub whirlpool_token_owner_account_a: UncheckedAccount<'info>,
    /// The payer's token account to deposit token_0
    #[account(
        mut,
        token::mint = gamma_token_0_vault.mint,
        token::authority = gamma_owner
    )]
    pub gamma_token_0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    // /// CHECK: Token owner account B
    // pub whirlpool_token_owner_account_b: UncheckedAccount<'info>,
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
    /// token Program
    pub token_program: Program<'info, Token>,

    /// Token program 2022
    pub token_program_2022: Program<'info, Token2022>,

    // /// CHECK: Token mint A
    // pub whirlpool_token_mint_a: UncheckedAccount<'info>,
    /// The mint of token_0 vault
    #[account(
        address = gamma_token_0_vault.mint
    )]
    pub gamma_vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    // /// CHECK: Token mint B
    // pub whirlpool_token_mint_b: UncheckedAccount<'info>,
    /// The mint of token_1 vault
    #[account(
        address = gamma_token_1_vault.mint
    )]
    pub gamma_vault_1_mint: Box<InterfaceAccount<'info, Mint>>,
    // remaining accounts
    // - accounts for transfer hook program of token_mint_a
    // - accounts for transfer hook program of token_mint_b
}


pub fn orca_whirlpool_to_gamma<'info>(
    ctx: Context<'_, '_, '_, 'info, OrcaWhirlpoolToGamma<'info>>,
    liquidity_amount: u128,
    token_min_a: u64,
    token_min_b: u64,
    maximum_token_0_amount: u64,
    maximum_token_1_amount: u64,
) -> Result<()> { 
    let user_token0_balance_before = ctx.accounts.gamma_token_0_account.amount;
    let user_token1_balance_before = ctx.accounts.gamma_token_1_account.amount;

    // Withdraw from Orca Whirlpool
    let accounts = whirlpool_cpi::cpi::accounts::DecreaseLiquidity {
        whirlpool: ctx.accounts.whirlpool.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
        position_authority: ctx.accounts.gamma_owner.to_account_info(),
        position: ctx.accounts.whirlpool_position.to_account_info(),
        position_token_account: ctx.accounts.whirlpool_position_token_account.to_account_info(),
        token_owner_account_a: ctx.accounts.gamma_token_0_account.to_account_info(),
        token_owner_account_b: ctx.accounts.gamma_token_1_account.to_account_info(),
        token_vault_a: ctx.accounts.whirlpool_token_vault_a.to_account_info(),
        token_vault_b: ctx.accounts.whirlpool_token_vault_b.to_account_info(),
        tick_array_lower: ctx.accounts.whirlpool_tick_array_lower.to_account_info(),
        tick_array_upper: ctx.accounts.whirlpool_tick_array_upper.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(ctx.accounts.whirlpool_program.to_account_info(), accounts);
    whirlpool_cpi::cpi::decrease_liquidity(cpi_ctx, liquidity_amount, token_min_a, token_min_b)?;
    
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

    deposit_to_gamma_pool(&mut deposit_accounts, gamma_lp_tokens as u64, maximum_token_0_amount, maximum_token_1_amount)?;
    
    emit!(MigrationEvent {
        from_pool: ctx.accounts.whirlpool.key(),
        to_pool: ctx.accounts.gamma_pool_state.key(),
        token_0_amount_withdrawn,
        token_1_amount_withdrawn,
        lp_tokens_migrated: gamma_lp_tokens,
    });
    
    Ok(())
}
