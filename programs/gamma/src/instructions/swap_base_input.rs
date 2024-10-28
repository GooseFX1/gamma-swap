use crate::curve::calculator::CurveCalculator;
use crate::curve::TradeDirection;
use crate::error::GammaError;
use crate::states::oracle;
use crate::states::AmmConfig;
use crate::states::ObservationState;
use crate::states::PoolState;
use crate::states::PoolStatusBitIndex;
use crate::states::SwapEvent;
use crate::utils::{swap_referral::*, token::*};
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

#[derive(Accounts)]
pub struct Swap<'info> {
    /// The user performing the swap
    pub payer: Signer<'info>,

    /// CHECK: pool vault and lp mint authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    /// The factory state to read protocol fees
    #[account(address = pool_state.load()?.amm_config)]
    pub amm_config: Box<Account<'info, AmmConfig>>,

    /// The program account of the pool in which the swap will be performed
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// The user token account for input token
    #[account(mut)]
    pub input_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The user token account for output token
    #[account(mut)]
    pub output_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The vault token account for input token
    #[account(
        mut,
        constraint = input_vault.key() == pool_state.load()?.token_0_vault || input_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub input_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The vault token account for output token
    #[account(
        mut,
        constraint = output_vault.key() == pool_state.load()?.token_0_vault || output_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub output_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// SPL program for input token transfers
    pub input_token_program: Interface<'info, TokenInterface>,

    /// SPL program for output token transfers
    pub output_token_program: Interface<'info, TokenInterface>,

    /// The mint of input token
    #[account(
        address = input_vault.mint
    )]
    pub input_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of output token
    #[account(
        address = output_vault.mint
    )]
    pub output_token_mint: Box<InterfaceAccount<'info, Mint>>,
    /// The program account for the most recent oracle observation
    #[account(mut, address = pool_state.load()?.observation_key)]
    pub observation_state: AccountLoader<'info, ObservationState>,
}

pub fn swap_base_input<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Swap<'info>>,
    amount_in: u64,
    minimum_amount_out: u64,
) -> Result<()> {
    let referral_info = extract_referral_info(
        ctx.accounts.input_token_mint.key(),
        ctx.accounts.amm_config.referral_project,
        &ctx.remaining_accounts,
    )?;
    let block_timestamp = solana_program::clock::Clock::get()?.unix_timestamp as u64;
    let pool_id = ctx.accounts.pool_state.key();
    let pool_state = &mut ctx.accounts.pool_state.load_mut()?;
    if !pool_state.get_status_by_bit(PoolStatusBitIndex::Swap)
        || block_timestamp < pool_state.open_time
    {
        return err!(GammaError::NotApproved);
    }

    let (token_0_price_x64_before_swap, token_1_price_x64_before_swap) =
        if ctx.accounts.input_vault.key() == pool_state.token_0_vault
            && ctx.accounts.output_vault.key() == pool_state.token_1_vault
        {
            pool_state.token_price_x32(
                ctx.accounts.input_vault.amount,
                ctx.accounts.output_vault.amount,
            )?
        } else if ctx.accounts.input_vault.key() == pool_state.token_1_vault
            && ctx.accounts.output_vault.key() == pool_state.token_0_vault
        {
            pool_state.token_price_x32(
                ctx.accounts.output_vault.amount,
                ctx.accounts.input_vault.amount,
            )?
        } else {
            return err!(GammaError::InvalidVault);
        };

    let transfer_fee =
        get_transfer_fee(&ctx.accounts.input_token_mint.to_account_info(), amount_in)?;
    // Take transfer fees into account for actual amount transferred in
    let actual_amount_in = amount_in.saturating_sub(transfer_fee);
    require_gt!(actual_amount_in, 0);

    // Calculate the trade amounts
    let (trade_direction, total_input_token_amount, total_output_token_amount) =
        if ctx.accounts.input_vault.key() == pool_state.token_0_vault
            && ctx.accounts.output_vault.key() == pool_state.token_1_vault
        {
            let (total_input_token_amount, total_output_token_amount) = pool_state
                .vault_amount_without_fee(
                    ctx.accounts.input_vault.amount,
                    ctx.accounts.output_vault.amount,
                )?;

            (
                TradeDirection::ZeroForOne,
                total_input_token_amount,
                total_output_token_amount,
            )
        } else if ctx.accounts.input_vault.key() == pool_state.token_1_vault
            && ctx.accounts.output_vault.key() == pool_state.token_0_vault
        {
            let (total_output_token_amount, total_input_token_amount) = pool_state
                .vault_amount_without_fee(
                    ctx.accounts.output_vault.amount,
                    ctx.accounts.input_vault.amount,
                )?;

            (
                TradeDirection::OneForZero,
                total_input_token_amount,
                total_output_token_amount,
            )
        } else {
            return err!(GammaError::InvalidVault);
        };
    let constant_before = u128::from(total_input_token_amount)
        .checked_mul(u128::from(total_output_token_amount))
        .ok_or(GammaError::MathOverflow)?;

    let mut observation_state = ctx.accounts.observation_state.load_mut()?;

    let result = match CurveCalculator::swap_base_input(
        u128::from(actual_amount_in),
        u128::from(total_input_token_amount),
        u128::from(total_output_token_amount),
        ctx.accounts.amm_config.trade_fee_rate,
        ctx.accounts.amm_config.protocol_fee_rate,
        ctx.accounts.amm_config.fund_fee_rate,
        block_timestamp,
        &observation_state,
    ) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::ZeroTradingTokens),
    };

    let constant_after = u128::from(result.new_swap_source_amount)
        .checked_mul(u128::from(result.new_swap_destination_amount))
        .ok_or(GammaError::MathOverflow)?;
    #[cfg(feature = "enable-log")]
    msg!(
        "source_amount_swapped:{}, destination_amount_swapped:{},constant_before:{},constant_after:{}",
        result.source_amount_swapped,
        result.destination_amount_swapped,
        constant_before,
        constant_after
    );
    require_gte!(constant_after, constant_before);
    let source_amount_swapped = match u64::try_from(result.source_amount_swapped) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::MathOverflow),
    };
    require_eq!(source_amount_swapped, actual_amount_in);
    let (mut input_transfer_amount, input_transfer_fee) = (amount_in, transfer_fee);
    let (output_transfer_amount, output_transfer_fee) = {
        let amount_out = match u64::try_from(result.destination_amount_swapped) {
            Ok(value) => value,
            Err(_) => return err!(GammaError::MathOverflow),
        };
        let transfer_fee = get_transfer_fee(
            &ctx.accounts.output_token_mint.to_account_info(),
            amount_out,
        )?;
        let amount_received = amount_out
            .checked_sub(transfer_fee)
            .ok_or(GammaError::MathOverflow)?;
        require_gt!(amount_received, 0);
        require_gte!(
            amount_received,
            minimum_amount_out,
            GammaError::ExceededSlippage
        );
        (amount_out, transfer_fee)
    };

    let protocol_fee = match u64::try_from(result.protocol_fee) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::MathOverflow),
    };
    let fund_fee = match u64::try_from(result.fund_fee) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::MathOverflow),
    };
    let mut dynamic_fee = match u64::try_from(result.dynamic_fee) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::MathOverflow),
    };

    if let Some(info) = referral_info {
        let referral_amount = dynamic_fee
            .saturating_sub(protocol_fee)
            .saturating_sub(fund_fee)
            .checked_mul(info.share_bps as u64)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(10_000)
            .unwrap_or(0);

        if referral_amount != 0 {
            // subtract referral amount from dynamic fee and transfer amount
            dynamic_fee = dynamic_fee
                .checked_sub(referral_amount)
                .ok_or(GammaError::MathError)?;
            input_transfer_amount = input_transfer_amount
                .checked_sub(referral_amount)
                .ok_or(GammaError::MathError)?;

            anchor_spl::token_2022::transfer_checked(
                CpiContext::new(
                    ctx.accounts.input_token_program.to_account_info(),
                    anchor_spl::token_2022::TransferChecked {
                        from: ctx.accounts.input_token_account.to_account_info(),
                        to: info.referral_token_account.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                        mint: ctx.accounts.input_token_mint.to_account_info(),
                    },
                ),
                referral_amount,
                ctx.accounts.input_token_mint.decimals,
            )?;
        }
    }

    match trade_direction {
        TradeDirection::ZeroForOne => {
            pool_state.protocol_fees_token_0 = pool_state
                .protocol_fees_token_0
                .checked_add(protocol_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.fund_fees_token_0 = pool_state
                .fund_fees_token_0
                .checked_add(fund_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.cumulative_trade_fees_token_0 = pool_state
                .cumulative_trade_fees_token_0
                .checked_add((dynamic_fee) as u128)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.cumulative_volume_token_0 = pool_state
                .cumulative_volume_token_0
                .checked_add(actual_amount_in as u128)
                .ok_or(GammaError::MathOverflow)?;
        }
        TradeDirection::OneForZero => {
            pool_state.protocol_fees_token_1 = pool_state
                .protocol_fees_token_1
                .checked_add(protocol_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.fund_fees_token_1 = pool_state
                .fund_fees_token_1
                .checked_add(fund_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.cumulative_trade_fees_token_1 = pool_state
                .cumulative_trade_fees_token_1
                .checked_add((dynamic_fee) as u128)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.cumulative_volume_token_1 = pool_state
                .cumulative_volume_token_1
                .checked_add(actual_amount_in as u128)
                .ok_or(GammaError::MathOverflow)?;
        }
    };

    emit!(SwapEvent {
        pool_id,
        input_vault_before: total_input_token_amount,
        output_vault_before: total_output_token_amount,
        input_amount: match u64::try_from(result.source_amount_swapped) {
            Ok(value) => value,
            Err(_) => return err!(GammaError::MathOverflow),
        },
        output_amount: match u64::try_from(result.destination_amount_swapped) {
            Ok(value) => value,
            Err(_) => return err!(GammaError::MathOverflow),
        },
        input_transfer_fee,
        output_transfer_fee,
        base_input: true
    });
    transfer_from_user_to_pool_vault(
        ctx.accounts.payer.to_account_info(),
        ctx.accounts.input_token_account.to_account_info(),
        ctx.accounts.input_vault.to_account_info(),
        ctx.accounts.input_token_mint.to_account_info(),
        ctx.accounts.input_token_program.to_account_info(),
        input_transfer_amount,
        ctx.accounts.input_token_mint.decimals,
    )?;
    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.output_vault.to_account_info(),
        ctx.accounts.output_token_account.to_account_info(),
        ctx.accounts.output_token_mint.to_account_info(),
        ctx.accounts.output_token_program.to_account_info(),
        output_transfer_amount,
        ctx.accounts.output_token_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[pool_state.auth_bump]]],
    )?;

    observation_state.update(
        oracle::block_timestamp()?,
        token_0_price_x64_before_swap,
        token_1_price_x64_before_swap,
    )?;

    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}
