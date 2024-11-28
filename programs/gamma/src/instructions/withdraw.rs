use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, Token2022, TokenAccount};

use crate::curve::{CurveCalculator, RoundDirection};
use crate::states::{
    LpChangeEvent, PartnerType, PoolStatusBitIndex, UserPoolLiquidity, USER_POOL_LIQUIDITY_SEED,
};
use crate::utils::{get_transfer_fee, transfer_from_pool_vault_to_user};
use crate::{error::GammaError, states::PoolState};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    /// Owner of the liquidity provided
    pub owner: Signer<'info>,

    /// CHECK: pool vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    /// Pool state account
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// User pool liquidity account
    #[account(
        mut,
        seeds = [
            USER_POOL_LIQUIDITY_SEED.as_bytes(),
            pool_state.key().as_ref(),
            owner.key().as_ref(),
        ],
        bump,
    )]
    pub user_pool_liquidity: Account<'info, UserPoolLiquidity>,

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
        constraint = token_0_vault.key() == pool_state.load()?.token_0_vault
    )]
    pub token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The address that holds pool tokens for token_1
    #[account(
        mut,
        constraint = token_1_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// token Program
    pub token_program: Program<'info, Token>,

    /// Token program 2022
    pub token_program_2022: Program<'info, Token2022>,

    /// The mint of token_0 vault
    #[account(
        address = token_0_vault.mint
    )]
    pub vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of token_1 vault
    #[account(
        address = token_1_vault.mint
    )]
    pub vault_1_mint: Box<InterfaceAccount<'info, Mint>>,

    /// memo program
    /// CHECK:
    #[account(
        address = spl_memo::id()
    )]
    pub memo_program: UncheckedAccount<'info>,
}

pub fn withdraw(
    ctx: Context<Withdraw>,
    lp_token_amount: u64,
    minimum_token_0_amount: u64,
    minimum_token_1_amount: u64,
) -> Result<()> {
    // require_gt!(ctx.accounts.lp_mint.supply, 0);
    let pool_id = ctx.accounts.pool_state.key();
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Withdraw) {
        return err!(GammaError::NotApproved);
    }
    let (total_token_0_amount, total_token_1_amount) = pool_state.vault_amount_without_fee(
        ctx.accounts.token_0_vault.amount,
        ctx.accounts.token_1_vault.amount,
    )?;
    let results = CurveCalculator::lp_tokens_to_trading_tokens(
        u128::from(lp_token_amount),
        u128::from(pool_state.lp_supply),
        u128::from(total_token_0_amount),
        u128::from(total_token_1_amount),
        RoundDirection::Floor,
    )
    .ok_or(GammaError::ZeroTradingTokens)?;

    let token_0_amount = match u64::try_from(results.token_0_amount) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::MathOverflow),
    };
    let token_0_amount = std::cmp::min(total_token_0_amount, token_0_amount);
    let (receive_token_0_amount, token_0_transfer_fee) = {
        let transfer_fee =
            get_transfer_fee(&ctx.accounts.vault_0_mint.to_account_info(), token_0_amount)?;
        (
            token_0_amount
                .checked_sub(transfer_fee)
                .ok_or(GammaError::MathOverflow)?,
            transfer_fee,
        )
    };

    let token_1_amount = match u64::try_from(results.token_1_amount) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::MathOverflow),
    };
    let token_1_amount = std::cmp::min(total_token_1_amount, token_1_amount);
    let (receive_token_1_amount, token_1_transfer_fee) = {
        let transfer_fee =
            get_transfer_fee(&ctx.accounts.vault_1_mint.to_account_info(), token_1_amount)?;
        (
            token_1_amount
                .checked_sub(transfer_fee)
                .ok_or(GammaError::MathOverflow)?,
            transfer_fee,
        )
    };

    #[cfg(feature = "enable-log")]
    msg!(
        "results.token_0_amount;{}, results.token_1_amount:{},receive_token_0_amount:{},token_0_transfer_fee:{},
            receive_token_1_amount:{},token_1_transfer_fee:{}",
        results.token_0_amount,
        results.token_1_amount,
        receive_token_0_amount,
        token_0_transfer_fee,
        receive_token_1_amount,
        token_1_transfer_fee
    );
    emit!(LpChangeEvent {
        pool_id,
        lp_amount_before: pool_state.lp_supply,
        token_0_vault_before: total_token_0_amount,
        token_1_vault_before: total_token_1_amount,
        token_0_amount: receive_token_0_amount,
        token_1_amount: receive_token_1_amount,
        token_0_transfer_fee,
        token_1_transfer_fee,
        change_type: 1
    });

    if receive_token_0_amount < minimum_token_0_amount
        || receive_token_1_amount < minimum_token_1_amount
    {
        return Err(GammaError::ExceededSlippage.into());
    }

    pool_state.lp_supply = pool_state
        .lp_supply
        .checked_sub(lp_token_amount)
        .ok_or(GammaError::MathOverflow)?;
    let user_pool_liquidity = &mut ctx.accounts.user_pool_liquidity;
    user_pool_liquidity.lp_tokens_owned = user_pool_liquidity
        .lp_tokens_owned
        .checked_sub(u128::from(lp_token_amount))
        .ok_or(GammaError::MathOverflow)?;
    user_pool_liquidity.token_0_withdrawn = user_pool_liquidity
        .token_0_withdrawn
        .checked_add(u128::from(receive_token_0_amount))
        .ok_or(GammaError::MathOverflow)?;
    user_pool_liquidity.token_1_withdrawn = user_pool_liquidity
        .token_1_withdrawn
        .checked_add(u128::from(receive_token_1_amount))
        .ok_or(GammaError::MathOverflow)?;

    if let Some(user_pool_liquidity_partner) = user_pool_liquidity.partner {
        let mut pool_state_partners = pool_state.partners;
        let partner: Option<&mut crate::states::PartnerInfo> = pool_state_partners
            .iter_mut()
            .find(|p| PartnerType::new(p.partner_id) == user_pool_liquidity_partner);
        if let Some(partner) = partner {
            partner.lp_token_linked_with_partner = partner
                .lp_token_linked_with_partner
                .checked_sub(lp_token_amount)
                .ok_or(GammaError::MathOverflow)?;
        }
        pool_state.partners = pool_state_partners;
    }

    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_0_vault.to_account_info(),
        ctx.accounts.token_0_account.to_account_info(),
        ctx.accounts.vault_0_mint.to_account_info(),
        if ctx.accounts.vault_0_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        token_0_amount,
        ctx.accounts.vault_0_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
    )?;

    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_1_vault.to_account_info(),
        ctx.accounts.token_1_account.to_account_info(),
        ctx.accounts.vault_1_mint.to_account_info(),
        if ctx.accounts.vault_1_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        token_1_amount,
        ctx.accounts.vault_1_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
    )?;
    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}
