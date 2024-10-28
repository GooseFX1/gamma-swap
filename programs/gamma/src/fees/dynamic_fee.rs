use super::{ceil_div, FEE_RATE_DENOMINATOR_VALUE};
use crate::{
    error::GammaError,
    states::ObservationState,
};
use anchor_lang::prelude::*;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use rust_decimal::MathematicalOps; // For ln()
//pub const FEE_RATE_DENOMINATOR_VALUE: u64 = 1_000_000;

// Volatility-based fee constants
pub const MAX_FEE_VOLATILITY: u64 = 10000; // 1% max fee
pub const VOLATILITY_WINDOW: u64 = 3600; // 1 hour window for volatility calculation

// Rebalancing-focused fee constants
pub const MIN_FEE_REBALANCE: u64 = 10_000; // 0.1% min fee /100_000
pub const MAX_FEE_REBALANCE: u64 = 100_000; // 10% max fee
pub const MID_FEE_REBALANCE: u64 = 26_000; // 2.6% mid fee
pub const OUT_FEE_REBALANCE: u64 = 50_000; // 5% out fee

const MAX_FEE: u64 = 100000; // 10% max fee
const VOLATILITY_FACTOR: u64 = 30_000; // Adjust based on desired sensitivity
const IMBALANCE_FACTOR: u64 = 20_000; // Adjust based on desired sensitivity

pub enum FeeType {
    Volatility,
}

pub struct DynamicFee {}

impl DynamicFee {
    /// Calculates a dynamic fee based on price volatility and liquidity imbalance
    ///
    /// # Arguments
    /// * `pool_state` - The current state of the pool
    /// * `observation_state` - Historical price observations
    /// * `vault_0` - Amount of token 0 in the vault
    /// * `vault_1` - Amount of token 1 in the vault
    ///
    /// # Returns
    /// A fee rate as a u64, where 10000 represents 1%
    pub fn calculate_volatile_fee(
        block_timestamp: u64,
        observation_state: &ObservationState,
        vault_0: u128,
        vault_1: u128,
        base_fees: u64,
    ) -> Result<u64> {
        // 1. Price volatility: (max_price - min_price) / avg_price
        // 2. Volatility component: min(VOLATILITY_FACTOR * volatility, MAX_FEE - BASE_FEE)
        // 3. Liquidity imbalance: |current_ratio - ideal_ratio|
        // 4. Imbalance component: IMBALANCE_FACTOR * imbalance / FEE_RATE_DENOMINATOR_VALUE
        // 5. Final fee: min(BASE_FEE + volatility_component + imbalance_component, MAX_FEE)

        // Calculate recent price volatility
        let (min_price, max_price, twap_price) =
            Self::get_price_range(observation_state, block_timestamp, VOLATILITY_WINDOW)?;
        // Handle case where no valid observations were found
        if min_price == 0 || max_price == 0 || twap_price == 0 {
            return Ok(base_fees);
        }

        // Convert prices to Decimal for logarithmic calculations
        let max_price_decimal = Decimal::from_u128(max_price).ok_or(GammaError::MathOverflow)?;
        let min_price_decimal = Decimal::from_u128(min_price).ok_or(GammaError::MathOverflow)?;
        let twap_price_decimal = Decimal::from_u128(twap_price).ok_or(GammaError::MathOverflow)?;

        // Compute logarithms
        let log_max_price = max_price_decimal.ln();
        let log_min_price = min_price_decimal.ln();
        let log_twap_price = twap_price_decimal.ln().abs();

        // Compute volatility numerator and denominator
        let volatility_numerator = (log_max_price - log_min_price).abs();
        let volatility_denominator = log_twap_price;

        // Check if volatility_denominator is zero to avoid division by zero
        if volatility_denominator.is_zero() {
            return Ok(base_fees);
        }

        // Compute volatility: volatility = volatility_numerator / volatility_denominator
        let volatility = volatility_numerator
            .checked_div(volatility_denominator)
            .ok_or(GammaError::MathOverflow)?;

        // Convert volatility to u64 scaled by FEE_RATE_DENOMINATOR_VALUE
        let scaled_volatility = (volatility * Decimal::from_u64(FEE_RATE_DENOMINATOR_VALUE)
            .ok_or(GammaError::MathOverflow)?)
            .to_u64()
            .ok_or(GammaError::MathOverflow)?;

        // Calculate volatility component
        let volatility_component_calculated = VOLATILITY_FACTOR
            .saturating_mul(scaled_volatility)
            .checked_div(FEE_RATE_DENOMINATOR_VALUE)
            .ok_or(GammaError::MathOverflow)?;

        let volatility_component = std::cmp::min(
            volatility_component_calculated,
            MAX_FEE
                .checked_sub(base_fees)
                .ok_or(GammaError::MathOverflow)?,
        );

        // Calculate liquidity imbalance component
        let total_liquidity = vault_0
            .checked_add(vault_1)
            .ok_or(GammaError::MathOverflow)? as u128;

        let current_ratio = if total_liquidity > 0 {
            (vault_0 as u128)
                .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
                .and_then(|product| product.checked_div(total_liquidity))
                .unwrap_or(0)
        } else {
            0
        };

        let ideal_ratio = FEE_RATE_DENOMINATOR_VALUE
            .checked_div(2)
            .ok_or(GammaError::MathOverflow)? as u128;

        let imbalance = if current_ratio > ideal_ratio {
            current_ratio.saturating_sub(ideal_ratio)
        } else {
            ideal_ratio.saturating_sub(current_ratio)
        };

        let liquidity_imbalance_component = IMBALANCE_FACTOR
            .saturating_mul(imbalance as u64)
            .checked_div(FEE_RATE_DENOMINATOR_VALUE)
            .unwrap_or(0);
        // Calculate final dynamic fee
        let dynamic_fee = base_fees
            .checked_add(volatility_component)
            .ok_or(GammaError::MathOverflow)?
            .checked_add(liquidity_imbalance_component)
            .ok_or(GammaError::MathOverflow)?;
        #[cfg(feature = "enable-log")]
        msg!("dynamic_fee: {}", dynamic_fee);
        Ok(std::cmp::min(dynamic_fee, MAX_FEE))
    }

    /// Calculates the dynamic fee based on the specified fee type
    ///
    /// # Arguments
    /// * `pool_state` - The current state of the pool
    /// * `observation_state` - Historical price observations
    /// * `vault_0` - Amount of token 0 in the vault
    /// * `vault_1` - Amount of token 1 in the vault
    /// * `fee_type` - The type of fee calculation to use
    ///
    /// # Returns
    /// A fee rate as a u64, where 10000 represents 1%
    pub fn calculate_dynamic_fee(
        block_timestamp: u64,
        observation_state: &ObservationState,
        vault_0: u128,
        vault_1: u128,
        fee_type: FeeType,
        base_fees: u64,
    ) -> Result<u64> {
        match fee_type {
            FeeType::Volatility => Self::calculate_volatile_fee(
                block_timestamp,
                observation_state,
                vault_0,
                vault_1,
                base_fees,
            ),
        }
    }

    /// Calculates a fee based on price volatility over a given time window
    ///
    /// # Arguments
    /// * `observation_state` - Historical price observations
    ///
    /// # Returns
    /// A fee rate as a u64, where 10000 represents 1%
    pub fn calculate_volatility_fee(
        block_timestamp: u64,
        observation_state: &ObservationState,
        base_fees: u64,
    ) -> Result<u64> {
        // 1. Calculate price range: (price_a, price_b)
        // 2. Volatility = |price_b - price_a| / min(price_a, price_b) * FEE_RATE_DENOMINATOR_VALUE
        // 3. Dynamic fee = min(volatility / 100 + BASE_FEE_VOLATILITY, MAX_FEE_VOLATILITY)

        let (price_a, price_b, _) =
            Self::get_price_range(observation_state, block_timestamp, VOLATILITY_WINDOW)?;
        let volatility = if price_b > price_a {
            price_b
                .checked_sub(price_a)
                .ok_or(GammaError::MathOverflow)?
                .checked_div(price_a)
                .ok_or(GammaError::MathOverflow)?
                .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
                .ok_or(GammaError::MathOverflow)?
        } else {
            price_a
                .checked_sub(price_b)
                .ok_or(GammaError::MathOverflow)?
                .checked_div(price_b)
                .ok_or(GammaError::MathOverflow)?
                .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
                .ok_or(GammaError::MathOverflow)?
        };

        let dynamic_fee = volatility
            .checked_div(100)
            .ok_or(GammaError::MathOverflow)?
            .checked_add(base_fees as u128)
            .ok_or(GammaError::MathOverflow)?; // Increase fee by 1 bp for each 1% of volatility
        Ok(dynamic_fee.min(MAX_FEE_VOLATILITY as u128) as u64)
    }

    /// Gets the price range within a specified time window and computes TWAP
    ///
    /// # Arguments
    /// * `observation_state` - Historical price observations
    /// * `current_time` - The current timestamp
    /// * `window` - The time window to consider
    ///
    /// # Returns
    /// A tuple of (min_price, max_price, twap_price) observed within the window
    fn get_price_range(
        observation_state: &ObservationState,
        current_time: u64,
        window: u64,
    ) -> Result<(u128, u128, u128)> {
        let mut min_price = u128::MAX;
        let mut max_price = 0u128;
        let mut weighted_price_sum = Decimal::new(0, 0);
        let mut total_weight = Decimal::new(0, 0);

        // Collect valid observations within the window
        let observations = observation_state
            .observations
            .iter()
            .filter(|obs| {
                obs.block_timestamp != 0
                    && obs.cumulative_token_0_price_x32 != 0
                    && current_time.saturating_sub(obs.block_timestamp) <= window
            })
            .collect::<Vec<_>>();

        if observations.len() < 2 {
            // Not enough data points to compute TWAP
            return Ok((0, 0, 0));
        }

        // Iterate over observation pairs to compute TWAP
        for i in 0..observations.len() - 1 {
            let obs = observations[i];
            let next_obs = observations[i + 1];

            let time_delta = next_obs
                .block_timestamp
                .saturating_sub(obs.block_timestamp) as u128;

            // Ensure time_delta is positive
            if time_delta == 0 {
                continue;
            }

            // Calculate price over the interval
            let price = next_obs
                .cumulative_token_0_price_x32
                .checked_sub(obs.cumulative_token_0_price_x32)
                .ok_or(GammaError::MathOverflow)?
                .checked_div(time_delta)
                .ok_or(GammaError::MathOverflow)?;

            // Update min and max prices
            min_price = min_price.min(price);
            max_price = max_price.max(price);

            // Accumulate weighted prices for TWAP
            let price_decimal = Decimal::from_u128(price).ok_or(GammaError::MathOverflow)?;
            let time_delta_decimal =
                Decimal::from_u128(time_delta).ok_or(GammaError::MathOverflow)?;
            weighted_price_sum = weighted_price_sum + (price_decimal * time_delta_decimal);
            total_weight = total_weight + time_delta_decimal;
        }

        if total_weight.is_zero() {
            // Avoid division by zero
            return Ok((0, 0, 0));
        }

        // Compute TWAP
        let twap_price_decimal = weighted_price_sum
            .checked_div(total_weight)
            .ok_or(GammaError::MathOverflow)?;

        let twap_price = twap_price_decimal
            .to_u128()
            .ok_or(GammaError::MathOverflow)?;

        Ok((min_price, max_price, twap_price))
    }

    /// Calculates the fee amount for a given input amount
    ///
    /// # Arguments
    /// * `amount` - The input amount
    /// * `pool_state` - The current state of the pool
    /// * `observation_state` - Historical price observations
    /// * `vault_0` - Amount of token 0 in the vault
    /// * `vault_1` - Amount of token 1 in the vault
    /// * `fee_type` - The type of fee calculation to use
    ///
    /// # Returns
    /// The fee amount as a u128, or None if calculation fails

    pub fn dynamic_fee(
        amount: u128,
        block_timestamp: u64,
        observation_state: &ObservationState,
        vault_0: u128,
        vault_1: u128,
        fee_type: FeeType,
        base_fees: u64,
    ) -> Result<u128> {
        let dynamic_fee_rate = Self::calculate_dynamic_fee(
            block_timestamp,
            observation_state,
            vault_0,
            vault_1,
            fee_type,
            base_fees,
        )?;

        Ok(ceil_div(
            amount,
            u128::from(dynamic_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
        .ok_or(GammaError::MathOverflow)?)
    }

    /// Calculates the pre-fee amount given a post-fee amount
    ///
    /// # Arguments
    /// * `post_fee_amount` - The amount after fees have been deducted
    /// * `pool_state` - The current state of the pool
    /// * `observation_state` - Historical price observations
    /// * `vault_0` - Amount of token 0 in the vault
    /// * `vault_1` - Amount of token 1 in the vault
    /// * `fee_type` - The type of fee calculation to use
    ///
    /// # Returns
    /// The pre-fee amount as a u128, or None if calculation fails
    pub fn calculate_pre_fee_amount(
        block_timestamp: u64,
        post_fee_amount: u128,
        observation_state: &ObservationState,
        vault_0: u128,
        vault_1: u128,
        fee_type: FeeType,
        base_fees: u64,
    ) -> Result<u128> {
        // x = pre_fee_amount (has to be calculated)
        // y = post_fee_amount
        // r = trade_fee_rate
        // D = FEE_RATE_DENOMINATOR_VALUE
        // y = x * (1 - r/ D)
        // y = x * ((D -r) / D)
        // x = y * D / (D - r)

        // Let x = pre_fee_amount, y = post_fee_amount, r = dynamic_fee_rate, D = FEE_RATE_DENOMINATOR_VALUE
        // y = x * (1 - r/D)
        // y = x * ((D - r) / D)
        // x = y * D / (D - r)
        // To avoid rounding errors, we use:
        // x = (y * D + (D - r) - 1) / (D - r)

        let dynamic_fee_rate = Self::calculate_dynamic_fee(
            block_timestamp,
            observation_state,
            vault_0,
            vault_1,
            fee_type,
            base_fees,
        )?;
        if dynamic_fee_rate == 0 {
            Ok(post_fee_amount)
        } else {
            let numerator = post_fee_amount
                .checked_mul(u128::from(FEE_RATE_DENOMINATOR_VALUE))
                .ok_or(GammaError::MathOverflow)?;
            let denominator = u128::from(FEE_RATE_DENOMINATOR_VALUE)
                .checked_sub(u128::from(dynamic_fee_rate))
                .ok_or(GammaError::MathOverflow)?;

            let result = numerator
                .checked_add(denominator)
                .ok_or(GammaError::MathOverflow)?
                .checked_sub(1)
                .ok_or(GammaError::MathOverflow)?
                .checked_div(denominator)
                .ok_or(GammaError::MathOverflow)?;

            Ok(result)
        }
    }
}
