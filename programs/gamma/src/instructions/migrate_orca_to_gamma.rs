use anchor_lang::prelude::*;

use crate::states::{AmmConfig, PoolState, POOL_SEED};

use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_2022::spl_token_2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use whirlpools::cpi::accounts::DecreaseLiquidity;
use whirlpools::program::Whirlpools;
use whirlpools::state::Whirlpool;

#[derive(Accounts)]
pub struct MigrateOrcaToGamma<'info> {
    /// Which amm config the pool belongs to
    pub amm_config: Box<Account<'info, AmmConfig>>,

    #[account(
        mut,
        seeds = [
            POOL_SEED.as_bytes(),
            amm_config.key().as_ref(),
            token_0_mint.key().as_ref(),
            token_1_mint.key().as_ref(),
        ],
        bump,
    )]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// Token_0 mint, the key must smaller than token_1 mint.
    #[account(
        constraint = token_0_mint.key() < token_1_mint.key(),
        mint::token_program = token_0_program,
    )]
    pub token_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Token_1 mint, the key must greater than token_0 mint.
    #[account(
        mint::token_program = token_1_program,
    )]
    pub token_1_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Spl token program or token program 2022
    pub token_0_program: Interface<'info, TokenInterface>,
    /// Spl token program or token program 2022
    pub token_1_program: Interface<'info, TokenInterface>,

    /// Orca Whirlpool account
    #[account(mut)]
    pub whirlpool: Account<'info, Whirlpool>,

    /// Liquidity position account
    #[account(mut)]
    pub position: Account<'info, TokenAccount>,

    /// Token accounts to receive the withdrawn liquidity
    #[account(mut)]
    pub token_0_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub token_1_account: Account<'info, TokenAccount>,

    /// Token program
    pub token_program: Program<'info, Token>,
    pub whirlpools_program: Program<'info, Whirlpools>,

}

pub fn migrate(ctx: Context<MigrateOrcaToGamma>, liquidity: u64) -> Result<()> {
    // Decrease liquidity in Orca Whirlpool
    let cpi_accounts = DecreaseLiquidity {
        whirlpool: ctx.accounts.whirlpool.to_account_info(),
        position: ctx.accounts.position.to_account_info(),
        token_0_account: ctx.accounts.token_0_account.to_account_info(),
        token_1_account: ctx.accounts.token_1_account.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
    };

    let cpi_program = ctx.accounts.whirlpools_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

    whirlpools::cpi::decrease_liquidity(cpi_ctx, liquidity)?;

    // Deposit the withdrawn tokens into Gamma pools

    let pool_id = ctx.accounts.pool_state.key();
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Deposit) {
        return err!(GammaError::NotApproved);
    }
    let (total_token_0_amount, total_token_1_amount) = pool_state.vault_amount_without_fee(
        ctx.accounts.token_0_vault.amount,
        ctx.accounts.token_1_vault.amount,
    );
    let results = CurveCalculator::lp_tokens_to_trading_tokens(
        u128::from(lp_token_amount),
        u128::from(pool_state.lp_supply),
        u128::from(total_token_0_amount),
        u128::from(total_token_1_amount),
        RoundDirection::Ceiling,
    )
    .ok_or(GammaError::ZeroTradingTokens)?;

    let token_0_amount = u64::try_from(results.token_0_amount).unwrap();
    let (transfer_token_0_amount, transfer_token_0_fee) = {
        let transfer_fee =
            get_transfer_inverse_fee(&ctx.accounts.vault_0_mint.to_account_info(), token_0_amount)?;
        (
            token_0_amount.checked_add(transfer_fee).unwrap(),
            transfer_fee,
        )
    };

    let token_1_amount = u64::try_from(results.token_1_amount).unwrap();
    let (transfer_token_1_amount, transfer_token_1_fee) = {
        let transfer_fee =
            get_transfer_inverse_fee(&ctx.accounts.vault_1_mint.to_account_info(), token_1_amount)?;
        (
            token_1_amount.checked_add(transfer_fee).unwrap(),
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
        pool_id,
        lp_amount_before: pool_state.lp_supply,
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

    pool_state.lp_supply = pool_state.lp_supply.checked_add(lp_token_amount).unwrap();
    let user_pool_liquidity = &mut ctx.accounts.user_pool_liquidity;
    user_pool_liquidity.token_0_deposited = user_pool_liquidity
        .token_0_deposited
        .checked_add(u128::from(transfer_token_0_amount))
        .unwrap();
    user_pool_liquidity.token_1_deposited = user_pool_liquidity
        .token_1_deposited
        .checked_add(u128::from(transfer_token_1_amount))
        .unwrap();
    user_pool_liquidity.lp_tokens_owned = user_pool_liquidity
        .lp_tokens_owned
        .checked_add(u128::from(lp_token_amount))
        .unwrap();
    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}