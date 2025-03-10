use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, Token2022, TokenAccount};

use crate::curve::{CurveCalculator, RoundDirection};
use crate::external::kamino::KaminoProgram;
use crate::states::{
    LpChangeEvent, PartnerType, PoolStatusBitIndex, UserPoolLiquidity, POOL_KAMINO_DEPOSITS_SEED,
    USER_POOL_LIQUIDITY_SEED,
};
use crate::utils::{get_transfer_fee, transfer_from_pool_vault_to_user};
use crate::{error::GammaError, states::PoolState};
use anchor_lang::solana_program::sysvar::instructions::ID as INSTRUCTION_SYSVAR_ID;

use super::calculate_amount_to_be_withdrawn_from_kamino_in_withdraw_instruction_in_liquidity_tokens;

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

    pub kamino_program: Program<'info, KaminoProgram>,

    #[account(address = INSTRUCTION_SYSVAR_ID )]
    /// CHECK: The native instructions sysvar
    pub instruction_sysvar_account: UncheckedAccount<'info>,
}

pub fn withdraw<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Withdraw<'info>>,
    lp_token_amount: u64,
    minimum_token_0_amount: u64,
    minimum_token_1_amount: u64,
) -> Result<()>
where
    'c: 'info,
{
    // require_gt!(ctx.accounts.lp_mint.supply, 0);
    let pool_id = ctx.accounts.pool_state.key();
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Withdraw) {
        return err!(GammaError::NotApproved);
    }
    let (total_token_0_amount, total_token_1_amount) = pool_state.vault_amount_without_fee()?;
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

    if receive_token_0_amount < minimum_token_0_amount
        || receive_token_1_amount < minimum_token_1_amount
    {
        return Err(GammaError::ExceededSlippage.into());
    }

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

    let end_index = withdraw_from_kamino_if_needed(&ctx, pool_state, token_0_amount, true, 0)?;
    withdraw_from_kamino_if_needed(&ctx, pool_state, token_1_amount, false, end_index)?;

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

    pool_state.token_0_vault_amount = pool_state
        .token_0_vault_amount
        .checked_sub(token_0_amount)
        .ok_or(GammaError::MathOverflow)?;
    pool_state.token_1_vault_amount = pool_state
        .token_1_vault_amount
        .checked_sub(token_1_amount)
        .ok_or(GammaError::MathOverflow)?;

    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}

#[derive(Accounts)]
pub struct RemainingKaminoAccounts<'info> {
    /// Account is checked in cpi
    /// CHECK: kamino reserve token 1
    #[account(mut)]
    pub kamino_reserve: AccountInfo<'info>,

    /// CHECK: The account address is checked in the cpi.
    #[account(mut)]
    pub kamino_lending_market: AccountInfo<'info>,

    /// CHECK: The account address is checked in the cpi.
    #[account()]
    pub lending_market_authority: AccountInfo<'info>,

    /// CHECK: The account address is checked in the cpi.
    #[account(mut)]
    pub reserve_liquidity_supply: AccountInfo<'info>,

    /// CHECK: The account address is checked in the cpi.
    #[account(mut)]
    pub reserve_collateral_mint: AccountInfo<'info>,

    /// CHECK: The account address is checked in the cpi.
    #[account(
        mut,
        token::mint = reserve_collateral_mint,
    )]
    pub gamma_pool_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
}

// Returns the end index of the remaining accounts,
// Any future reads to remaining accounts should start from that index.
pub fn withdraw_from_kamino_if_needed<'c, 'info>(
    ctx: &Context<'_, '_, 'c, 'info, Withdraw<'info>>,
    pool_state: &mut PoolState,
    token_amount_being_withdrawn: u64,
    token0_or_token1: bool,
    start_index: usize,
) -> Result<usize>
where
    'c: 'info,
{
    let remaining_accounts = &ctx.remaining_accounts;
    let token_vault = match token0_or_token1 {
        true => &ctx.accounts.token_0_vault,
        false => &ctx.accounts.token_1_vault,
    };

    let amount_to_withdraw_from_kamino_in_liquidity_tokens =
        calculate_amount_to_be_withdrawn_from_kamino_in_withdraw_instruction_in_liquidity_tokens(
            &pool_state,
            token_amount_being_withdrawn,
            token_vault,
        )?;

    if amount_to_withdraw_from_kamino_in_liquidity_tokens == 0 {
        return Ok(0);
    }

    let kamino_accounts = RemainingKaminoAccounts {
        kamino_reserve: remaining_accounts[start_index].to_account_info(),
        kamino_lending_market: remaining_accounts[start_index + 1].to_account_info(),
        lending_market_authority: remaining_accounts[start_index + 2].to_account_info(),
        reserve_liquidity_supply: remaining_accounts[start_index + 3].to_account_info(),
        reserve_collateral_mint: remaining_accounts[start_index + 4].to_account_info(),
        gamma_pool_destination_collateral: Box::new(InterfaceAccount::try_from(
            &remaining_accounts[start_index + 5],
        )?),
    };

    // Verify gamma_pool_destination_collateral seeds are correct
    let pool_state_key = ctx.accounts.pool_state.key();
    let reserve_liquidity_mint = match token0_or_token1 {
        true => ctx.accounts.vault_0_mint.to_account_info(),
        false => ctx.accounts.vault_1_mint.to_account_info(),
    };
    let expected_seeds = [
        POOL_KAMINO_DEPOSITS_SEED.as_bytes(),
        pool_state_key.as_ref(),
        reserve_liquidity_mint.key.as_ref(),
    ];
    let pubkey_derived =
        Pubkey::find_program_address(&expected_seeds, &ctx.accounts.kamino_program.key()).0;
    if pubkey_derived != kamino_accounts.gamma_pool_destination_collateral.key() {
        return err!(ErrorCode::ConstraintSeeds);
    }

    let signer_seeds: &[&[&[u8]]] = &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]];

    let liquidity_token_program =
        if token_vault.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        };

    let kamino_withdraw_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.kamino_program.to_account_info(),
        crate::external::kamino::kamino::cpi::accounts::RedeemReserveCollateral {
            owner: ctx.accounts.authority.to_account_info(),
            reserve: kamino_accounts.kamino_reserve.to_account_info(),
            lending_market: kamino_accounts.kamino_lending_market,
            reserve_liquidity_mint,
            reserve_liquidity_supply: kamino_accounts.reserve_liquidity_supply,
            lending_market_authority: kamino_accounts.lending_market_authority,
            reserve_collateral_mint: kamino_accounts.reserve_collateral_mint,
            user_source_collateral: kamino_accounts
                .gamma_pool_destination_collateral
                .to_account_info(),
            user_destination_liquidity: token_vault.to_account_info(),
            collateral_token_program: ctx.accounts.token_program.to_account_info(),
            liquidity_token_program,
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.to_account_info(),
        },
        signer_seeds,
    );

    let amount_in_collateral_tokens = crate::external::kamino::liquidity_to_collateral(
        &kamino_accounts.kamino_reserve,
        amount_to_withdraw_from_kamino_in_liquidity_tokens,
    )?;

    crate::external::kamino::kamino::cpi::redeem_reserve_collateral(
        kamino_withdraw_cpi_ctx,
        amount_in_collateral_tokens,
    )?;

    // The withdrawn amount is not profit, we profit is only withdrawn in rebalance instructions.
    // It is not profit because the amount to withdraw is using the current amounts in the pool, including all the profit collected by the pool.
    if token0_or_token1 {
        pool_state.token_0_amount_in_kamino = pool_state
            .token_0_amount_in_kamino
            .checked_sub(amount_to_withdraw_from_kamino_in_liquidity_tokens)
            .ok_or(GammaError::MathOverflow)?;
    } else {
        pool_state.token_1_amount_in_kamino = pool_state
            .token_1_amount_in_kamino
            .checked_sub(amount_to_withdraw_from_kamino_in_liquidity_tokens)
            .ok_or(GammaError::MathOverflow)?;
    }

    Ok(start_index + 6)
}
