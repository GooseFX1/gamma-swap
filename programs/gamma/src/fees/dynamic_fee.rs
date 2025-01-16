use super::{ceil_div, FEE_RATE_DENOMINATOR_VALUE};
use crate::{
    error::GammaError,
    fees::ONE_BASIS_POINT,
    states::{Observation, ObservationState, PoolState, OBSERVATION_NUM},
};
use anchor_lang::prelude::*;
use rust_decimal::prelude::*;

// Volatility-based fee constants
pub const MAX_FEE_VOLATILITY: u64 = 10000; // 1% max fee
pub const VOLATILITY_WINDOW: u64 = 3600; // 1 hour window for volatility calculation

const DEFAULT_MAX_FEE: u64 = 100000; // 10% max fee
const DEFAULT_VOLATILITY_FACTOR: u64 = 300_000; // Adjust based on desired sensitivity

pub enum FeeType {
    Volatility,
}

struct ObservationWithIndex {
    observation: Observation,
    index: u16,
}

pub struct DynamicFee {}

impl DynamicFee {
    /// Calculates the fee amount for a given input amount (base_fees + dynamic_fee)
    ///
    /// # Arguments
    /// * `amount` - The input amount
    /// * `block_timestamp` - The current block timestamp
    /// * `observation_state` - Historical price observations
    /// * `fee_type` - The type of fee calculation to use
    /// * `base_fees` - The base fee rate
    ///
    /// # Returns
    /// The fee amount as a u128, or None if calculation fails

    pub fn dynamic_fee(
        amount: u128,
        block_timestamp: u64,
        observation_state: &ObservationState,
        fee_type: FeeType,
        base_fees: u64,
        pool_state: &PoolState,
        is_invoked_by_signed_segmenter: bool,
    ) -> Result<(u128, u64)> {
        // TODO: use is_invoked_by_signed_segmenter to charge less fees for signed segmenter, once they are implemented across all protocols and this is also used by the segmenter.
        let dynamic_fee_rate = Self::calculate_dynamic_fee(
            block_timestamp,
            observation_state,
            fee_type,
            base_fees,
            pool_state,
            is_invoked_by_signed_segmenter,
        )?;

        Ok((
            ceil_div(
                amount,
                u128::from(dynamic_fee_rate),
                u128::from(FEE_RATE_DENOMINATOR_VALUE),
            )
            .ok_or(GammaError::MathOverflow)?,
            dynamic_fee_rate,
        ))
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
    fn calculate_dynamic_fee(
        block_timestamp: u64,
        observation_state: &ObservationState,
        fee_type: FeeType,
        base_fees: u64,
        pool_state: &PoolState,
        is_invoked_by_signed_segmenter: bool,
    ) -> Result<u64> {
        match fee_type {
            FeeType::Volatility => Self::calculate_volatile_fee(
                block_timestamp,
                observation_state,
                base_fees,
                pool_state,
                is_invoked_by_signed_segmenter,
            ),
        }
    }

    /// Calculates a dynamic fee based on price volatility
    ///
    /// # Arguments
    /// * `block_timestamp` - The current block timestamp
    /// * `observation_state` - Historical price observations
    /// * `base_fees` - The base fee rate
    ///
    /// # Returns
    /// A fee rate as a u64, where 10000 represents 1%
    fn calculate_volatile_fee(
        block_timestamp: u64,
        observation_state: &ObservationState,
        base_fees: u64,
        pool_state: &PoolState,
        is_invoked_by_signed_segmenter: bool,
    ) -> Result<u64> {
        // 1. Price volatility calculation:
        //    - Get min, max and TWAP (Time-Weighted Average Price) over the volatility window
        //    - Volatility = |ln(max_price) - ln(min_price)| / |ln(twap_price)|
        //
        // 2. Volatility component calculation:
        //    - Scale volatility by FEE_RATE_DENOMINATOR_VALUE (1_000_000)
        //    - Multiply by VOLATILITY_FACTOR (30_000) to adjust sensitivity
        //    - Cap the component at (MAX_FEE - BASE_FEE) to ensure total fee doesn't exceed MAX_FEE
        //
        // 3. Final fee calculation:
        //    - Add base_fees to volatility_component
        //    - Ensure final fee doesn't exceed MAX_FEE (100_000 = 10%)
        //    - Result is a fee rate where 10_000 represents 1%

        let (min_price, max_price, twap_price) =
            Self::get_price_range(observation_state, block_timestamp, VOLATILITY_WINDOW)?;
        // Handle case where no valid observations were found
        if min_price == 0 || max_price == 0 || twap_price == 0 || twap_price == 1 {
            // If twap is 1 we will get ln(1) = 0, so we can't divide by 0
            return Ok(base_fees);
        }

        // Compute logarithms
        let log_max_price = (max_price as f64).ln();
        let log_min_price = (min_price as f64).ln();
        let log_twap_price = (twap_price as f64).ln();
        #[cfg(feature = "enable-log")]
        msg!(
            "log_max_price: {},log_min_price={},log_twap_price={}  ",
            log_max_price,
            log_min_price,
            log_twap_price
        );

        // Compute volatility numerator and denominator
        let volatility_numerator = (log_max_price - log_min_price).abs();
        let volatility_denominator = log_twap_price.abs();

        // Check if volatility_denominator is zero to avoid division by zero
        if volatility_denominator.is_zero() {
            return Ok(base_fees);
        }

        // Compute volatility: volatility = volatility_numerator / volatility_denominator
        // Dividing f64 with f64. We want to know the decimals so we keep the
        let volatility = volatility_numerator / volatility_denominator;
        #[cfg(feature = "enable-log")]
        msg!("volatility: {} ", volatility);

        #[cfg(feature = "enable-log")]
        msg!(
            "is_invoked_by_signed_segmenter: {}",
            is_invoked_by_signed_segmenter
        );

        let volatility_factor = if pool_state.volatility_factor == 0 {
            DEFAULT_VOLATILITY_FACTOR
        } else {
            pool_state.volatility_factor
        };

        // Calculate volatility component
        let volatility_component_calculated = (volatility_factor as f64 * volatility)
            .to_u64()
            .ok_or(GammaError::MathOverflow)?;
        #[cfg(feature = "enable-log")]
        msg!(
            "volatility_component_calculated: {} ",
            volatility_component_calculated
        );

        // Calculate final dynamic fee
        let dynamic_fee = base_fees
            .checked_add(volatility_component_calculated)
            .ok_or(GammaError::MathOverflow)?;

        let max_fee = if pool_state.max_trade_fee_rate == 0 {
            DEFAULT_MAX_FEE
        } else {
            pool_state.max_trade_fee_rate
        };

        #[cfg(feature = "enable-log")]
        msg!("dynamic_fee: {}", dynamic_fee);
        let mut final_fee = std::cmp::min(dynamic_fee, max_fee);
        if is_invoked_by_signed_segmenter && final_fee > 10 * ONE_BASIS_POINT {
            final_fee = final_fee - ONE_BASIS_POINT;
        }

        Ok(final_fee)
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
        // Filter and sort observations:
        // 1. Remove invalid observations (zero timestamps or prices)
        // 2. Keep only observations within our time window
        // 3. Sort from newest to oldest
        let mut descending_order_observations = observation_state
            .observations
            .iter()
            .enumerate()
            .filter(|(_, observation)| {
                observation.block_timestamp != 0
                    && observation.cumulative_token_0_price_x32 != 0
                    && observation.cumulative_token_1_price_x32 != 0
                    && current_time.saturating_sub(observation.block_timestamp) <= window
            })
            .map(|(index, observation)| ObservationWithIndex {
                index: index as u16,
                observation: *observation,
            })
            .collect::<Vec<_>>();

        // Sort observations by timestamp (newest first)
        descending_order_observations.sort_by(|a, b| {
            { b.observation.block_timestamp }.cmp(&{ a.observation.block_timestamp })
        });

        // Need at least 2 observations to calculate prices
        if descending_order_observations.len() < 2 {
            // Not enough data points to compute TWAP
            return Ok((0, 0, 0));
        }

        // For TWAP: use first and last observations within our window
        // Get newest and oldest observations in our filtered set
        let newest_obs = descending_order_observations.first().unwrap();
        let oldest_obs = descending_order_observations.last().unwrap();

        // Calculate time delta using the correct start time
        let total_time_delta = newest_obs
            .observation
            .block_timestamp
            .saturating_sub(oldest_obs.observation.block_timestamp)
            as u128;

        if total_time_delta == 0 {
            return Ok((0, 0, 0));
        }

        // Calculate TWAP using real observations only
        let twap_price = newest_obs
            .observation
            .cumulative_token_0_price_x32
            .checked_sub(oldest_obs.observation.cumulative_token_0_price_x32)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(total_time_delta)
            .ok_or(GammaError::MathOverflow)?;

        // Iterate to find min/max spot prices
        for observation_with_index in descending_order_observations {
            let last_observation_index = if observation_with_index.index == 0 {
                OBSERVATION_NUM - 1
            } else {
                observation_with_index.index as usize - 1
            };

            // if last observation is not valid, skip this observation
            if observation_state.observations[last_observation_index].block_timestamp == 0 {
                continue;
            }

            if observation_state.observations[last_observation_index].block_timestamp
                > observation_with_index.observation.block_timestamp
            {
                // Break if current observation is older than the last observation.
                break;
            }

            let obs = observation_state.observations[last_observation_index];
            let next_obs = observation_with_index.observation;

            let time_delta = next_obs.block_timestamp.saturating_sub(obs.block_timestamp) as u128;

            if time_delta == 0 {
                continue;
            }

            // Calculate spot price for this interval
            let price = next_obs
                .cumulative_token_0_price_x32
                .checked_sub(obs.cumulative_token_0_price_x32)
                .ok_or(GammaError::MathOverflow)?
                .checked_div(time_delta)
                .ok_or(GammaError::MathOverflow)?;

            // Update min and max prices
            min_price = min_price.min(price);
            max_price = max_price.max(price);
        }

        Ok((min_price, max_price, twap_price))
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
        fee_type: FeeType,
        base_fees: u64,
        pool_state: &PoolState,
        is_invoked_by_signed_segmenter: bool,
    ) -> Result<(u128, u64)> {
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
            fee_type,
            base_fees,
            pool_state,
            is_invoked_by_signed_segmenter,
        )?;
        if dynamic_fee_rate == 0 {
            Ok((post_fee_amount, 0))
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

            Ok((result, dynamic_fee_rate))
        }
    }
}
