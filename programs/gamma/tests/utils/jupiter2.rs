use anchor_lang::AccountDeserialize;
use anyhow::{anyhow, Context, Result};
use gamma::curve::{ConstantProductCurve, CurveCalculator, SwapResult, TradeDirection};
use gamma::fees::{ceil_div, DynamicFee, FeeType, StaticFee, FEE_RATE_DENOMINATOR_VALUE};
use gamma::states::{ObservationState, PoolStatusBitIndex};
use jupiter_amm_interface::{
    try_get_account_data, AccountMap, Amm, AmmContext, KeyedAccount, Quote, QuoteParams,
    SwapAndAccountMetas, SwapParams,
};
use rust_decimal::prelude::FromPrimitive;
use spl_token_2022::extension::BaseStateWithExtensions;
use spl_token_2022::extension::{
    transfer_fee::TransferFeeConfig, StateWithExtensions, StateWithExtensionsOwned,
};
use spl_token_2022::state::Mint;
use std::sync::atomic::{AtomicI64, AtomicU64};
use std::sync::Arc;

use anchor_lang::ToAccountMetas;
use gamma::{
    states::{AmmConfig, PoolState},
    AUTH_SEED,
};
use solana_sdk::pubkey::Pubkey;

#[derive(Clone)]
pub struct TokenMints {
    token0: Pubkey,
    token1: Pubkey,
    token0_mint: StateWithExtensionsOwned<Mint>,
    token1_mint: StateWithExtensionsOwned<Mint>,
    token0_program: Pubkey,
    token1_program: Pubkey,
}

#[derive(Clone)]
pub struct Gamma {
    key: Pubkey,
    pool_state: PoolState,
    amm_config: Option<AmmConfig>,
    vault_0_amount: Option<u64>,
    vault_1_amount: Option<u64>,
    token_mints_and_token_programs: Option<TokenMints>,
    epoch: Arc<AtomicU64>,
    timestamp: Arc<AtomicI64>,
    observation_state: Option<ObservationState>,
}

impl Gamma {
    fn get_authority(&self) -> Pubkey {
        Pubkey::create_program_address(
            &[AUTH_SEED.as_bytes(), &[self.pool_state.auth_bump]],
            &gamma::ID,
        )
        .unwrap()
    }
}

impl Amm for Gamma {
    fn from_keyed_account(keyed_account: &KeyedAccount, amm_context: &AmmContext) -> Result<Self> {
        let pool_state = PoolState::try_deserialize(&mut keyed_account.account.data.as_ref())?;

        Ok(Self {
            key: keyed_account.key,
            pool_state,
            amm_config: None,
            vault_0_amount: None,
            vault_1_amount: None,
            token_mints_and_token_programs: None,
            epoch: amm_context.clock_ref.epoch.clone(),
            timestamp: amm_context.clock_ref.unix_timestamp.clone(),
            observation_state: None,
        })
    }

    fn label(&self) -> String {
        "GAMMA".into()
    }

    fn program_id(&self) -> Pubkey {
        gamma::id()
    }

    fn key(&self) -> Pubkey {
        self.key
    }

    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        vec![self.pool_state.token_0_mint, self.pool_state.token_1_mint]
    }

    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        let mut keys = vec![
            self.key,
            self.pool_state.token_0_vault,
            self.pool_state.token_1_vault,
            self.pool_state.amm_config,
        ];
        keys.extend([self.pool_state.token_0_mint, self.pool_state.token_1_mint]);
        keys
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let pool_state_data = try_get_account_data(account_map, &self.key)?;
        self.pool_state = PoolState::try_deserialize(&mut pool_state_data.as_ref())?;

        let token0_mint = try_get_account_data(account_map, &self.pool_state.token_0_mint)
            .ok()
            .and_then(|account_data| {
                StateWithExtensionsOwned::<spl_token_2022::state::Mint>::unpack(
                    account_data.to_vec(),
                )
                .ok()
            })
            .context("Token 0 mint not found")?;

        let token1_mint = try_get_account_data(account_map, &self.pool_state.token_1_mint)
            .ok()
            .and_then(|account_data| {
                StateWithExtensionsOwned::<spl_token_2022::state::Mint>::unpack(
                    account_data.to_vec(),
                )
                .ok()
            })
            .context("Token 1 mint not found")?;

        self.token_mints_and_token_programs = Some(TokenMints {
            token0: self.pool_state.token_0_mint,
            token1: self.pool_state.token_1_mint,
            token0_mint,
            token1_mint,
            token0_program: self.pool_state.token_0_program,
            token1_program: self.pool_state.token_1_program,
        });

        let amm_config_data = try_get_account_data(account_map, &self.pool_state.amm_config)?;
        self.amm_config = Some(AmmConfig::try_deserialize(&mut amm_config_data.as_ref())?);

        let get_unfrozen_token_amount = |token_vault| {
            try_get_account_data(account_map, token_vault)
                .ok()
                .and_then(|account_data| {
                    StateWithExtensions::<spl_token_2022::state::Account>::unpack(account_data).ok()
                })
                .and_then(|token_account| {
                    if token_account.base.is_frozen() {
                        None
                    } else {
                        Some(token_account.base.amount)
                    }
                })
        };

        let observation_state =
            try_get_account_data(account_map, &self.pool_state.observation_key)?;
        self.observation_state = Some(ObservationState::try_deserialize(
            &mut observation_state.as_ref(),
        )?);

        self.vault_0_amount = get_unfrozen_token_amount(&self.pool_state.token_0_vault);
        self.vault_1_amount = get_unfrozen_token_amount(&self.pool_state.token_1_vault);

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        if !self.pool_state.get_status_by_bit(PoolStatusBitIndex::Swap)
            || (self.timestamp.load(std::sync::atomic::Ordering::Relaxed) as u64)
                < self.pool_state.open_time
        {
            return Err(anyhow!("Pool is not trading"));
        }

        let amm_config = self.amm_config.as_ref().context("Missing AmmConfig")?;

        let zero_for_one: bool = quote_params.input_mint == self.pool_state.token_0_mint;

        if self.token_mints_and_token_programs.is_none() {
            return Err(anyhow!("Missing token mints and token programs"));
        }

        let TokenMints {
            token0_mint: token_mint_0,
            token1_mint: token_mint_1,
            ..
        } = self
            .token_mints_and_token_programs
            .as_ref()
            .ok_or(anyhow!("Missing token mints and token programs"))?;

        let token_mint_0_transfer_fee_config: Option<_> =
            token_mint_0.get_extension::<TransferFeeConfig>().ok();
        let token_mint_1_transfer_fee_config =
            token_mint_1.get_extension::<TransferFeeConfig>().ok();

        let (source_mint_transfer_fee_config, destination_mint_transfer_fee_config) =
            if zero_for_one {
                (
                    token_mint_0_transfer_fee_config,
                    token_mint_1_transfer_fee_config,
                )
            } else {
                (
                    token_mint_1_transfer_fee_config,
                    token_mint_0_transfer_fee_config,
                )
            };

        let amount = quote_params.amount;
        let epoch = self.epoch.load(std::sync::atomic::Ordering::Relaxed);

        let actual_amount_in = if let Some(transfer_fee_config) = source_mint_transfer_fee_config {
            amount.saturating_sub(
                transfer_fee_config
                    .calculate_epoch_fee(epoch, amount)
                    .context("Fee calculation failure")?,
            )
        } else {
            amount
        };
        if actual_amount_in == 0 {
            return Err(anyhow!("Amount too low"));
        }

        // Calculate the trade amounts
        let (total_token_0_amount, total_token_1_amount) =
            vault_amount_without_fee(&self.pool_state)?;

        let result = OracleBasedSwapCalculator::swap_base_input(
            actual_amount_in.into(),
            if zero_for_one {
                total_token_0_amount.into()
            } else {
                total_token_1_amount.into()
            },
            if zero_for_one {
                total_token_1_amount.into()
            } else {
                total_token_0_amount.into()
            },
            &amm_config,
            &self.pool_state,
            self.timestamp.load(std::sync::atomic::Ordering::Relaxed) as u64,
            self.observation_state
                .as_ref()
                .context("Missing observation state")?,
            false,
        )
        .context("swap failed")?;

        let amount_out: u64 = result.destination_amount_swapped.try_into()?;
        let actual_amount_out =
            if let Some(transfer_fee_config) = destination_mint_transfer_fee_config {
                amount_out.saturating_sub(
                    transfer_fee_config
                        .calculate_epoch_fee(epoch, amount_out)
                        .context("Fee calculation failure")?,
                )
            } else {
                amount_out
            };

        Ok(Quote {
            in_amount: actual_amount_in,
            out_amount: actual_amount_out,
            fee_mint: quote_params.input_mint,
            fee_amount: result.dynamic_fee as u64,
            // our understanding is this is the fee percentage of the input amount
            fee_pct: rust_decimal::Decimal::from_u128(result.dynamic_fee)
                .ok_or(anyhow!("Math overflow"))?
                .checked_div(
                    rust_decimal::Decimal::from_u64(actual_amount_in)
                        .ok_or(anyhow!("Math overflow"))?,
                )
                .context("Failed to divide")?,
            ..Default::default()
        })
    }

    fn get_accounts_len(&self) -> usize {
        14
    }

    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas> {
        if self.token_mints_and_token_programs.is_none() {
            return Err(anyhow!("Missing token mints and token programs"));
        }

        let TokenMints {
            token0_program: token_0_token_program,
            token1_program: token_1_token_program,
            ..
        } = self
            .token_mints_and_token_programs
            .as_ref()
            .ok_or(anyhow!("Missing token mints and token programs"))?;

        let (
            input_token_program,
            input_vault,
            input_token_mint,
            output_token_program,
            output_vault,
            output_token_mint,
        ) = if swap_params.source_mint == self.pool_state.token_0_mint {
            (
                *token_0_token_program,
                self.pool_state.token_0_vault,
                self.pool_state.token_0_mint,
                *token_1_token_program,
                self.pool_state.token_1_vault,
                self.pool_state.token_1_mint,
            )
        } else {
            (
                *token_1_token_program,
                self.pool_state.token_1_vault,
                self.pool_state.token_1_mint,
                *token_0_token_program,
                self.pool_state.token_0_vault,
                self.pool_state.token_0_mint,
            )
        };

        let account_metas = gamma::accounts::Swap {
            payer: swap_params.token_transfer_authority,
            authority: self.get_authority(),
            amm_config: self.pool_state.amm_config,
            pool_state: self.key,
            input_token_account: swap_params.source_token_account,
            output_token_account: swap_params.destination_token_account,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
            input_token_mint,
            output_token_mint,
            observation_state: self.pool_state.observation_key,
        }
        .to_account_metas(None);
        // The discriminator for the new instruction is
        // "discriminator": [239, 82, 192, 187, 160, 26, 223, 223],
        // Everything else is the same as the old instruction.

        unimplemented!()
        // Ok(SwapAndAccountMetas {
        //     swap: Swap::Gamma, // TODO: Add Gamma as option.
        //     account_metas,
        // })
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }
}

// We are extracting this here to avoid the need to fix the contract it self.
// https://github.com/GooseFX1/gamma/blob/61105a2415831e61111b3d0bbcd7a830724ee5cb/programs/gamma/src/states/pool.rs#L161-L170
fn vault_amount_without_fee(pool: &PoolState) -> Result<(u64, u64)> {
    Ok((pool.token_0_vault_amount, pool.token_1_vault_amount))
}

// Price scaled to 9 decimal places
pub const D9: u128 = 1_000_000_000;
const D9_TIMES_D9: u128 = D9 * D9;

pub struct OracleBasedSwapCalculator {}

impl OracleBasedSwapCalculator {
    /// Get the amount to be swapped at oracle price without reaching the acceptable price difference.
    pub fn get_amount_to_be_swapped_at_oracle_price(
        source_amount_to_be_swapped: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        // If swap is happening from x->y price is y/x
        // If swap is happening from y->x Price is x/y
        oracle_price: u128,
        pool_state: &PoolState,
    ) -> Result<u128> {
        let max_amount_swappable_at_oracle_price = swap_source_amount
            .checked_mul(pool_state.max_amount_swappable_at_oracle_price.into())
            .ok_or(anyhow!("Math overflow"))?
            .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(anyhow!("Math overflow"))?;

        // Max amount that can be swapped without reaching the acceptable price difference limit
        let price_difference_limit = FEE_RATE_DENOMINATOR_VALUE
            .checked_sub(pool_state.acceptable_price_difference.into())
            .ok_or(anyhow!("Math overflow"))?;
        // We can swap with oracle price, P until we reach spot_price_at_acceptable_price_difference_limit Z
        // We want to calculate the spot_price_at_acceptable_price_difference_limit that is away from current oracle_price and not current spot_price.
        let spot_price_at_acceptable_price_difference_limit = oracle_price
            .checked_mul(price_difference_limit.into())
            .ok_or(anyhow!("Math overflow"))?
            .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(anyhow!("Math overflow"))?;

        // Max tradeable amount with price Oracle Price P before we reach spot_price_at_acceptable_price_difference_limit Z
        // Can we derived by the formula:
        // x_delta_max = (|(Z*X) - Y)| / (Z + P)
        let z_times_x = spot_price_at_acceptable_price_difference_limit
            .checked_mul(swap_source_amount)
            .ok_or(anyhow!("Math overflow"))?;
        let y_scaled_by_d9 = swap_destination_amount
            .checked_mul(D9)
            .ok_or(anyhow!("Math overflow"))?;

        // numerator = |(Z*X) - Y|
        let numerator = z_times_x.abs_diff(y_scaled_by_d9);
        // denominator = Z + P
        let denominator = oracle_price
            .checked_add(spot_price_at_acceptable_price_difference_limit)
            .ok_or(anyhow!("Math overflow"))?;

        let max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference =
            numerator
                .checked_div(denominator)
                .ok_or(anyhow!("Math overflow"))?;

        let max_swap_at_oracle_price = std::cmp::min(
            max_amount_swappable_at_oracle_price,
            max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference,
        );

        Ok(std::cmp::min(
            max_swap_at_oracle_price,
            source_amount_to_be_swapped,
        ))
    }

    pub fn get_spot_price_and_oracle_price_rate_difference(
        oracle_price: u128,
        spot_price: u128,
    ) -> Result<u128> {
        let difference_in_oracle_price = spot_price.abs_diff(oracle_price);
        let rate_difference = difference_in_oracle_price
            .checked_mul(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(anyhow!("Math overflow"))?
            .checked_div(oracle_price)
            .ok_or(anyhow!("Math overflow"))?;

        Ok(rate_difference)
    }

    pub fn get_execution_oracle_price(
        oracle_price: u128,
        price_premium_for_swap_at_oracle_price: u128,
    ) -> Result<u128> {
        let oracle_price_premium = oracle_price
            .checked_mul(price_premium_for_swap_at_oracle_price)
            .ok_or(anyhow!("Math overflow"))?
            .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(anyhow!("Math overflow"))?;

        // Make our price slightly better than the oracle price.
        let execution_oracle_price = oracle_price
            .checked_add(oracle_price_premium)
            .ok_or(anyhow!("Math overflow"))?;

        Ok(execution_oracle_price)
    }

    /// Subtract fees and calculate how much destination token will be received
    /// for a given amount of source token
    pub fn swap_base_input(
        source_amount_to_be_swapped: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        amm_config: &AmmConfig,
        pool_state: &PoolState,
        block_timestamp: u64,
        observation_state: &ObservationState,
        is_invoked_by_signed_segmenter: bool,
    ) -> Result<SwapResult> {
        let oracle_price_updated_at = pool_state.oracle_price_updated_at;
        let difference = block_timestamp.saturating_sub(oracle_price_updated_at);
        if difference > pool_state.max_oracle_price_update_time_diff as u64
            || block_timestamp < oracle_price_updated_at
            || oracle_price_updated_at == 0
            || pool_state.oracle_price_token_0_by_token_1 == 0
        {
            return Ok(CurveCalculator::swap_base_input(
                source_amount_to_be_swapped,
                swap_source_amount,
                swap_destination_amount,
                amm_config,
                pool_state,
                block_timestamp,
                observation_state,
                is_invoked_by_signed_segmenter,
            )?);
        }

        let vault_amounts = pool_state.vault_amount_without_fee()?;
        let trade_direction = if swap_source_amount == vault_amounts.0 as u128 {
            TradeDirection::ZeroForOne
        } else {
            TradeDirection::OneForZero
        };

        // We always take the price to be opposite of the trade direction
        // If swap is happening from x->y price is y/x
        // If swap is happening from y->x Price is x/y
        let spot_price = swap_destination_amount
            .checked_mul(D9)
            .ok_or(anyhow!("Math overflow"))?
            .checked_div(swap_source_amount)
            .ok_or(anyhow!("Math overflow"))?;

        let oracle_price = match trade_direction {
            TradeDirection::OneForZero => pool_state.oracle_price_token_0_by_token_1,
            TradeDirection::ZeroForOne => D9_TIMES_D9
                .checked_div(pool_state.oracle_price_token_0_by_token_1)
                .ok_or(anyhow!("Math overflow"))?,
        };

        let rate_difference =
            Self::get_spot_price_and_oracle_price_rate_difference(oracle_price, spot_price)?;
        if rate_difference > pool_state.acceptable_price_difference as u128 {
            // If the price difference between pool and oracle is too high, we will use the old calculator.
            return Ok(CurveCalculator::swap_base_input(
                source_amount_to_be_swapped,
                swap_source_amount,
                swap_destination_amount,
                amm_config,
                pool_state,
                block_timestamp,
                observation_state,
                is_invoked_by_signed_segmenter,
            )?);
        }

        let amount_to_be_swapped_at_oracle_price = Self::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            pool_state,
        )?;
        let amount_to_be_swapped_with_invariant_curve = source_amount_to_be_swapped
            .checked_sub(amount_to_be_swapped_at_oracle_price)
            .ok_or(anyhow!("Math overflow"))?;

        if amount_to_be_swapped_at_oracle_price == 0 {
            return Ok(CurveCalculator::swap_base_input(
                source_amount_to_be_swapped,
                swap_source_amount,
                swap_destination_amount,
                amm_config,
                pool_state,
                block_timestamp,
                observation_state,
                is_invoked_by_signed_segmenter,
            )?);
        }

        let dynamic_fee_rate = DynamicFee::dynamic_fee_rate(
            block_timestamp,
            observation_state,
            FeeType::Volatility,
            amm_config.trade_fee_rate,
            pool_state,
            is_invoked_by_signed_segmenter,
        )?;

        let trade_rate_on_amount_to_be_swapped_at_oracle_price = std::cmp::max(
            dynamic_fee_rate,
            pool_state.min_trade_rate_at_oracle_price.into(),
        );

        let trade_fees_for_oracle_swap = ceil_div(
            amount_to_be_swapped_at_oracle_price.into(),
            trade_rate_on_amount_to_be_swapped_at_oracle_price.into(),
            FEE_RATE_DENOMINATOR_VALUE.into(),
        )
        .ok_or(anyhow!("Math overflow"))?;

        let source_amount_to_be_swapped_after_fees = amount_to_be_swapped_at_oracle_price
            .checked_sub(trade_fees_for_oracle_swap)
            .ok_or(anyhow!("Math overflow"))?;

        let execution_oracle_price = Self::get_execution_oracle_price(
            oracle_price,
            pool_state.price_premium_for_swap_at_oracle_price.into(),
        )?;

        // The price is Y/X, we have delta_x, so to find y, we need to do y = delta_x * price
        // Since price was scaled by D9, we need to scale down by D9
        let output_tokens = execution_oracle_price
            .checked_mul(source_amount_to_be_swapped_after_fees)
            .ok_or(anyhow!("Math overflow"))?
            .checked_div(D9)
            .ok_or(anyhow!("Math overflow"))?;

        let new_swap_source_amount = swap_source_amount
            .checked_add(amount_to_be_swapped_at_oracle_price)
            .ok_or(anyhow!("Math overflow"))?;

        let new_swap_destination_amount = swap_destination_amount
            .checked_sub(output_tokens)
            .ok_or(anyhow!("Math overflow"))?;

        let trade_fees_for_invariant_curve = ceil_div(
            amount_to_be_swapped_with_invariant_curve.into(),
            dynamic_fee_rate.into(),
            FEE_RATE_DENOMINATOR_VALUE.into(),
        )
        .ok_or(anyhow!("Math overflow"))?;

        let source_amount_after_fees = amount_to_be_swapped_with_invariant_curve
            .checked_sub(trade_fees_for_invariant_curve)
            .ok_or(anyhow!("Math overflow"))?;
        let trade_fee_charged = trade_fees_for_invariant_curve
            .checked_add(trade_fees_for_oracle_swap)
            .ok_or(anyhow!("Math overflow"))?;

        let trade_fee_rate = trade_fee_charged
            .checked_mul(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(anyhow!("Math overflow"))?
            .checked_div(source_amount_to_be_swapped)
            .ok_or(anyhow!("Math overflow"))?;

        let destination_amount_swapped_with_curve_calculator =
            ConstantProductCurve::swap_base_input_without_fees(
                source_amount_after_fees,
                new_swap_source_amount,
                new_swap_destination_amount,
            )?;

        #[cfg(feature = "enable-log")]
        msg!(
            "trade_fee_charged: {}, trade_fee_rate: {}",
            trade_fee_charged,
            trade_fee_rate
        );
        let destination_amount_swapped = destination_amount_swapped_with_curve_calculator
            .checked_add(output_tokens)
            .ok_or(anyhow!("Math overflow"))?;

        let protocol_fee = StaticFee::protocol_fee(trade_fee_charged, amm_config.protocol_fee_rate)
            .ok_or(anyhow!("Invalid fee"))?;
        let fund_fee = StaticFee::fund_fee(trade_fee_charged, amm_config.fund_fee_rate)
            .ok_or(anyhow!("Invalid fee"))?;

        Ok(SwapResult {
            new_swap_source_amount: swap_source_amount
                .checked_add(source_amount_to_be_swapped)
                .ok_or(anyhow!("Math overflow"))?,
            new_swap_destination_amount: swap_destination_amount
                .checked_sub(destination_amount_swapped)
                .ok_or(anyhow!("Math overflow"))?,
            source_amount_swapped: source_amount_to_be_swapped,
            destination_amount_swapped,
            dynamic_fee: trade_fee_charged,
            protocol_fee,
            fund_fee,
            dynamic_fee_rate: trade_fee_rate as u64,
        })
    }
}
