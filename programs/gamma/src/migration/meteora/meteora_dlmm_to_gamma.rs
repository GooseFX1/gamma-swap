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
use dlmm_cpi::BinLiquidityReduction;

#[derive(Accounts)]
pub struct MeteoraDlmmToGamma<'info> {
    #[account(mut)]
    /// CHECK: The position account
    pub dlmm_position: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: The pool account
    pub dlmm_lb_pair: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Bin array extension account of the pool
    pub dlmm_bin_array_bitmap_extension: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    /// CHECK: Reserve account of token X
    pub dlmm_reserve_x: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Reserve account of token Y
    pub dlmm_reserve_y: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Bin array lower account
    pub dlmm_bin_array_lower: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Bin array upper account
    pub dlmm_bin_array_upper: UncheckedAccount<'info>,

    #[account(address = dlmm_cpi::ID)]
    /// CHECK: DLMM program
    pub dlmm_program: UncheckedAccount<'info>,

    /// CHECK: DLMM program event authority for event CPI
    pub dlmm_event_authority: UncheckedAccount<'info>,

    /// CHECK: Token program of mint X
    pub token_x_program: UncheckedAccount<'info>,
    /// CHECK: Token program of mint Y
    pub token_y_program: UncheckedAccount<'info>,

    // /// CHECK: User who is withdrawing from DLMM pool
    // pub dlmm_sender: Signer<'info>,
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

    // #[account(mut)]
    // ///CHECK: User's token x account
    // pub dlmm_user_token_x: UncheckedAccount<'info>,
    /// The payer's token account to deposit token_0
    #[account(
        mut,
        token::mint = gamma_token_0_vault.mint,
        token::authority = gamma_owner
    )]
    pub gamma_token_0_account: Box<InterfaceAccount<'info, TokenAccount>>,

    // #[account(mut)]
    // /// CHECK: User's token y account
    // pub dlmm_user_token_y: UncheckedAccount<'info>,
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

    /// CHECK: Mint account of token X
    // pub dlmm_token_x_mint: UncheckedAccount<'info>,
    /// The mint of token_0 vault
    #[account(
        address = gamma_token_0_vault.mint
    )]
    pub gamma_vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    // /// CHECK: Mint account of token Y
    // pub dlmm_token_y_mint: UncheckedAccount<'info>,
    /// The mint of token_1 vault
    #[account(
        address = gamma_token_1_vault.mint
    )]
    pub gamma_vault_1_mint: Box<InterfaceAccount<'info, Mint>>,
}

pub fn meteora_dlmm_to_gamma(
    ctx: Context<MeteoraDlmmToGamma>,
    bin_liquidity_reduction: Vec<BinLiquidityReduction>,
    maximum_token_0_amount: u64,
    maximum_token_1_amount: u64,
) -> Result<()> {
    let user_token0_balance_before = ctx.accounts.gamma_token_0_account.amount;
    let user_token1_balance_before = ctx.accounts.gamma_token_1_account.amount;
    // Withdraw from Meteora DLMM
    let accounts = dlmm_cpi::cpi::accounts::RemoveLiquidity {
        position: ctx.accounts.dlmm_position.to_account_info(),
        lb_pair: ctx.accounts.dlmm_lb_pair.to_account_info(),
        bin_array_bitmap_extension: if let Some(bin_array_bitmap_extension) =
            &ctx.accounts.dlmm_bin_array_bitmap_extension
        {
            bin_array_bitmap_extension.to_account_info()
        } else {
            ctx.accounts.dlmm_program.to_account_info()
        },
        bin_array_lower: ctx.accounts.dlmm_bin_array_lower.to_account_info(),
        bin_array_upper: ctx.accounts.dlmm_bin_array_upper.to_account_info(),
        user_token_x: ctx.accounts.gamma_token_0_account.to_account_info(),
        user_token_y: ctx.accounts.gamma_token_1_account.to_account_info(),
        reserve_x: ctx.accounts.dlmm_reserve_x.to_account_info(),
        reserve_y: ctx.accounts.dlmm_reserve_y.to_account_info(),
        token_x_mint: ctx.accounts.gamma_vault_0_mint.to_account_info(),
        token_y_mint: ctx.accounts.gamma_vault_1_mint.to_account_info(),
        sender: ctx.accounts.gamma_owner.to_account_info(),
        token_x_program: ctx.accounts.token_x_program.to_account_info(),
        token_y_program: ctx.accounts.token_y_program.to_account_info(),
        event_authority: ctx.accounts.dlmm_event_authority.to_account_info(),
        program: ctx.accounts.dlmm_program.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(ctx.accounts.dlmm_program.to_account_info(), accounts);
    dlmm_cpi::cpi::remove_liquidity(cpi_ctx, bin_liquidity_reduction)?;

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
        from_pool: ctx.accounts.dlmm_lb_pair.key(),
        to_pool: ctx.accounts.gamma_pool_state.key(),
        token_0_amount_withdrawn,
        token_1_amount_withdrawn,
        lp_tokens_migrated: gamma_lp_tokens,
    });

    Ok(())
}
