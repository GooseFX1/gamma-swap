use super::Swap;
use crate::curve::OracleBasedSwapCalculator;
use crate::curve::TradeDirection;
use crate::error::GammaError;
use crate::external::dflow_segmenter::is_invoked_by_segmenter;
use crate::fees::FEE_RATE_DENOMINATOR_VALUE;
use crate::instructions::SwapRemainingAccounts;
use crate::states::oracle;
use crate::states::PoolStatusBitIndex;
use crate::states::SwapEvent;
use crate::utils::{swap_referral::*, token::*};
use anchor_lang::prelude::*;
use anchor_lang::solana_program;

pub fn oracle_based_swap_base_input<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Swap<'info>>,
    amount_in: u64,
    minimum_amount_out: u64,
) -> Result<()> {
    let swap_remaining_accounts = SwapRemainingAccounts::new(&ctx.remaining_accounts);
    let referral_info = extract_referral_info(
        ctx.accounts.input_token_mint.key(),
        ctx.accounts.amm_config.referral_project,
        &swap_remaining_accounts.referral_account,
        &swap_remaining_accounts.referral_token_account,
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
            pool_state.token_price_x32()?
        } else if ctx.accounts.input_vault.key() == pool_state.token_1_vault
            && ctx.accounts.output_vault.key() == pool_state.token_0_vault
        {
            pool_state.token_price_x32()?
        } else {
            return err!(GammaError::InvalidVault);
        };

    let transfer_fee =
        get_transfer_fee(&ctx.accounts.input_token_mint.to_account_info(), amount_in)?;
    // Take transfer fees into account for actual amount transferred in
    let mut actual_amount_in = amount_in.saturating_sub(transfer_fee);
    require_gt!(actual_amount_in, 0);

    // Calculate the trade amounts
    let (trade_direction, total_input_token_amount, total_output_token_amount) =
        if ctx.accounts.input_vault.key() == pool_state.token_0_vault
            && ctx.accounts.output_vault.key() == pool_state.token_1_vault
        {
            let (total_input_token_amount, total_output_token_amount) =
                pool_state.vault_amount_without_fee()?;

            (
                TradeDirection::ZeroForOne,
                total_input_token_amount,
                total_output_token_amount,
            )
        } else if ctx.accounts.input_vault.key() == pool_state.token_1_vault
            && ctx.accounts.output_vault.key() == pool_state.token_0_vault
        {
            let (total_output_token_amount, total_input_token_amount) =
                pool_state.vault_amount_without_fee()?;

            (
                TradeDirection::OneForZero,
                total_input_token_amount,
                total_output_token_amount,
            )
        } else {
            return err!(GammaError::InvalidVault);
        };

    let mut observation_state = ctx.accounts.observation_state.load_mut()?;

    let mut is_invoked_by_signed_segmenter = false;

    if swap_remaining_accounts.registered_segmenter.is_some()
        && swap_remaining_accounts.registry.is_some()
    {
        is_invoked_by_signed_segmenter = is_invoked_by_segmenter(
            &swap_remaining_accounts.registry.as_ref().unwrap(),
            &swap_remaining_accounts
                .registered_segmenter
                .as_ref()
                .unwrap(),
        );
    }

    let result = match OracleBasedSwapCalculator::swap_base_input(
        u128::from(actual_amount_in),
        u128::from(total_input_token_amount),
        u128::from(total_output_token_amount),
        &ctx.accounts.amm_config,
        &pool_state,
        block_timestamp,
        &observation_state,
        is_invoked_by_signed_segmenter,
    ) {
        Ok(value) => value,
        Err(_) => return err!(GammaError::ZeroTradingTokens),
    };

    #[cfg(feature = "enable-log")]
    msg!(
        "actual_amount_in:{} source_amount_swapped:{}, destination_amount_swapped:{}, dynamic_fee: {}",
        actual_amount_in,
        result.source_amount_swapped,
        result.destination_amount_swapped,
        result.dynamic_fee,
        
    );
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

    let mut protocol_fee = u64::try_from(result.protocol_fee).or(err!(GammaError::MathOverflow))?;
    let mut fund_fee = u64::try_from(result.fund_fee).or(err!(GammaError::MathOverflow))?;
    let dynamic_fee = u64::try_from(result.dynamic_fee).or(err!(GammaError::MathOverflow))?;

    let mut transfer_referral_amount = None;
    if let Some(ref info) = referral_info {
        let referral_result_from_protocol_fee = info.get_referral_amount(protocol_fee)?;
        let referral_result_from_fund_fee = info.get_referral_amount(fund_fee)?;
        let referral_amount = referral_result_from_protocol_fee
            .referral_amount
            .checked_add(referral_result_from_fund_fee.referral_amount)
            .ok_or(GammaError::MathOverflow)?;

        let referral_transfer_fee = get_transfer_fee(
            &ctx.accounts.input_token_mint.to_account_info(),
            referral_amount,
        )?;

        #[cfg(feature = "enable-log")]
        msg!(
            "referral_amount:{}, referral_transfer_fee:{}",
            referral_amount,
            referral_transfer_fee
        );

        // We are aware of the fact that when referral fees are very small the referee will not get any tokens
        if referral_amount != 0 && referral_transfer_fee < referral_amount {
            protocol_fee = referral_result_from_protocol_fee.amount_after_referral;
            fund_fee = referral_result_from_fund_fee.amount_after_referral;

            // we subtract the input transfer amount that these tokens are directly transferred from user to lp pool.
            input_transfer_amount = input_transfer_amount
                .checked_sub(referral_amount)
                .ok_or(GammaError::MathOverflow)?;
            actual_amount_in = actual_amount_in
                .checked_sub(referral_amount)
                .ok_or(GammaError::MathOverflow)?;

            transfer_referral_amount = Some(referral_amount)
        }
    }

    let partner_protocol_fee_u128 = (pool_state.partner_share_rate as u128)
        .checked_mul(protocol_fee as u128)
        .ok_or(GammaError::MathOverflow)?
        .checked_div(FEE_RATE_DENOMINATOR_VALUE as u128)
        .ok_or(GammaError::MathOverflow)?;
    let partner_protocol_fee =
        u64::try_from(partner_protocol_fee_u128).map_err(|_| GammaError::MathError)?;

    protocol_fee = protocol_fee
        .checked_sub(partner_protocol_fee)
        .ok_or(GammaError::MathOverflow)?;

    match trade_direction {
        TradeDirection::ZeroForOne => {
            pool_state.protocol_fees_token_0 = pool_state
                .protocol_fees_token_0
                .checked_add(protocol_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.partner_protocol_fees_token_0 = pool_state
                .partner_protocol_fees_token_0
                .checked_add(partner_protocol_fee)
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
            pool_state.cumulative_volume_token_1 = pool_state
                .cumulative_volume_token_1
                .checked_add(output_transfer_amount as u128)
                .ok_or(GammaError::MathOverflow)?;

            pool_state.token_0_vault_amount = pool_state
                .token_0_vault_amount
                .checked_add(actual_amount_in)
                .ok_or(GammaError::MathOverflow)?
                .checked_sub(fund_fee)
                .ok_or(GammaError::MathOverflow)?
                .checked_sub(protocol_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.token_1_vault_amount = pool_state
                .token_1_vault_amount
                .checked_sub(output_transfer_amount)
                .ok_or(GammaError::MathOverflow)?;
        }
        TradeDirection::OneForZero => {
            pool_state.protocol_fees_token_1 = pool_state
                .protocol_fees_token_1
                .checked_add(protocol_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.partner_protocol_fees_token_1 = pool_state
                .partner_protocol_fees_token_1
                .checked_add(partner_protocol_fee)
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
            pool_state.cumulative_volume_token_0 = pool_state
                .cumulative_volume_token_0
                .checked_add(output_transfer_amount as u128)
                .ok_or(GammaError::MathOverflow)?;

            pool_state.token_1_vault_amount = pool_state
                .token_1_vault_amount
                .checked_add(actual_amount_in)
                .ok_or(GammaError::MathOverflow)?
                .checked_sub(fund_fee)
                .ok_or(GammaError::MathOverflow)?
                .checked_sub(protocol_fee)
                .ok_or(GammaError::MathOverflow)?;
            pool_state.token_0_vault_amount = pool_state
                .token_0_vault_amount
                .checked_sub(output_transfer_amount)
                .ok_or(GammaError::MathOverflow)?;
        }
    };
    pool_state.latest_dynamic_fee_rate = result.dynamic_fee_rate;

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
        input_mint: ctx.accounts.input_vault.mint,
        output_mint: ctx.accounts.output_vault.mint,
        input_transfer_fee,
        output_transfer_fee,
        base_input: true,
        dynamic_fee: result.dynamic_fee
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

    // Even though referral accounts are processed above, it's more convenient for
    // indexers to rely on the input and output token-transfer instructions having
    // a fixed inner-instruction index.
    // Hence:
    // (0) is user->vault token transfer,
    // (1) is vault->user token transfer,
    // (2) is(optionally) user->referrer token transfer
    if let Some(amount) = transfer_referral_amount {
        let info = referral_info.expect("referral_info to be non-null");
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
            amount,
            ctx.accounts.input_token_mint.decimals,
        )?;
    }

    observation_state.update(
        oracle::block_timestamp()?,
        token_0_price_x64_before_swap,
        token_1_price_x64_before_swap,
    )?;

    pool_state.recent_epoch = Clock::get()?.epoch;

    Ok(())
}
