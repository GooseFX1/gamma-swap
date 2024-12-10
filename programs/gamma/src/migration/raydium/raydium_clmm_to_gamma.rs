use crate::{
    calculate_gamma_lp_tokens,
    instructions::deposit::{deposit_to_gamma_pool, Deposit},
    states::{MigrationEvent, PoolState, UserPoolLiquidity, USER_POOL_LIQUIDITY_SEED},
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, Token2022, TokenAccount},
};
#[derive(Accounts)]
pub struct RaydiumClmmToGamma<'info> {
    #[account(address = clmm_cpi::ID)]
    /// CHECK: clmm program
    pub raydium_clmm_program: UncheckedAccount<'info>,
    /// CHECK: The position owner or delegated authority
    pub raydium_clmm_nft_owner: Signer<'info>,

    /// CHECK: The token account for the tokenized position
    #[account()]
    pub raydium_clmm_nft_account: UncheckedAccount<'info>,

    /// CHECK: Decrease liquidity for this position
    #[account(mut)]
    pub raydium_clmm_personal_position: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: pool state
    pub raydium_clmm_pool_state: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: protocol position
    pub raydium_clmm_protocol_position: UncheckedAccount<'info>,

    /// Token_0 vault
    #[account(mut)]
    pub raydium_clmm_token_vault_0: UncheckedAccount<'info>,

    /// CHECK: Token_1 vault
    #[account(mut)]
    pub raydium_clmm_token_vault_1: UncheckedAccount<'info>,

    /// CHECK: Stores init state for the lower tick
    #[account(mut)]
    pub raydium_clmm_tick_array_lower: UncheckedAccount<'info>,

    /// CHECK: Stores init state for the upper tick
    #[account(mut)]
    pub raydium_clmm_tick_array_upper: UncheckedAccount<'info>,

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
    // remaining account
    // #[account(
    //     seeds = [
    //         POOL_TICK_ARRAY_BITMAP_SEED.as_bytes(),
    //         pool_state.key().as_ref(),
    //     ],
    //     bump
    // )]
    // pub tick_array_bitmap: AccountLoader<'info, TickArrayBitmapExtension>,
    // pub tick_array_bitmap: UncheckedAccount<'info>,
}

pub fn raydium_clmm_to_gamma<'a, 'b, 'c, 'info>(
    ctx: Context<'a, 'b, 'c, 'info, RaydiumClmmToGamma<'info>>,
    liquidity: u128,
    amount_0_min: u64,
    amount_1_min: u64,
    maximum_token_0_amount: u64,
    maximum_token_1_amount: u64,
) -> Result<()> {
    let user_token0_balance_before = ctx.accounts.gamma_token_0_account.amount;
    let user_token1_balance_before = ctx.accounts.gamma_token_1_account.amount;

    // Withdraw from Raydium CLMM
    let cpi_accounts = clmm_cpi::cpi::accounts::DecreaseLiquidity {
        nft_owner: ctx.accounts.raydium_clmm_nft_owner.to_account_info(),
        nft_account: ctx.accounts.raydium_clmm_nft_account.to_account_info(),
        personal_position: ctx
            .accounts
            .raydium_clmm_personal_position
            .to_account_info(),
        pool_state: ctx.accounts.raydium_clmm_pool_state.to_account_info(),
        protocol_position: ctx
            .accounts
            .raydium_clmm_protocol_position
            .to_account_info(),
        token_vault0: ctx.accounts.raydium_clmm_token_vault_0.to_account_info(),
        token_vault1: ctx.accounts.raydium_clmm_token_vault_1.to_account_info(),
        tick_array_lower: ctx.accounts.raydium_clmm_tick_array_lower.to_account_info(),
        tick_array_upper: ctx.accounts.raydium_clmm_tick_array_upper.to_account_info(),
        recipient_token_account0: ctx.accounts.gamma_token_0_account.to_account_info(),
        recipient_token_account1: ctx.accounts.gamma_token_1_account.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
    };
    let cpi_context = CpiContext::new(
        ctx.accounts.raydium_clmm_program.to_account_info(),
        cpi_accounts,
    )
    .with_remaining_accounts(ctx.remaining_accounts.to_vec());
    clmm_cpi::cpi::decrease_liquidity(cpi_context, liquidity, amount_0_min, amount_1_min)?;

    ctx.accounts.gamma_token_0_account.reload()?;
    ctx.accounts.gamma_token_1_account.reload()?;

    let user_token0_balance_after = ctx.accounts.gamma_token_0_account.amount;
    let user_token1_balance_after = ctx.accounts.gamma_token_1_account.amount;
    let token_0_amount_withdrawn = user_token0_balance_before
        .checked_sub(user_token0_balance_after)
        .unwrap();
    let token_1_amount_withdrawn = user_token1_balance_before
        .checked_sub(user_token1_balance_after)
        .unwrap();
    let pool_state = ctx.accounts.gamma_pool_state.load()?;
    let gamma_lp_tokens = calculate_gamma_lp_tokens(
        token_0_amount_withdrawn,
        token_1_amount_withdrawn,
        &pool_state,
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

    deposit_to_gamma_pool(
        &mut deposit_accounts,
        gamma_lp_tokens as u64,
        maximum_token_0_amount,
        maximum_token_1_amount,
    )?;

    emit!(MigrationEvent {
        from_pool: ctx.accounts.raydium_clmm_pool_state.key(),
        to_pool: ctx.accounts.gamma_pool_state.key(),
        token_0_amount_withdrawn,
        token_1_amount_withdrawn,
        lp_tokens_migrated: gamma_lp_tokens,
    });

    Ok(())
}
