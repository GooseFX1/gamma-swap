use super::{ceil_div, FEE_RATE_DENOMINATOR_VALUE};
use crate::{
    error::GammaError,
    states::{Observation, ObservationState, OBSERVATION_NUM},
};
use anchor_lang::prelude::*;
//pub const FEE_RATE_DENOMINATOR_VALUE: u64 = 1_000_000;

// Volatility-based fee constants
pub const MAX_FEE_VOLATILITY: u64 = 10000; // 1% max fee
pub const VOLATILITY_WINDOW: u64 = 3600; // 1 hour window for volatility calculation

const MAX_FEE: u64 = 100000; // 10% max fee
const VOLATILITY_FACTOR: u64 = 30_000; // Adjust based on desired sensitivity

pub enum FeeType {
    Volatility,
}

struct ObservationWithIndex {
    observation: Observation,
    index: u16,
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
        base_fees: u64,
    ) -> Result<u64> {
        // 1. Price volatility: (max_price - min_price) / avg_price
        // 2. Volatility component: min(VOLATILITY_FACTOR * volatility, MAX_FEE - BASE_FEE)
        // 3. Liquidity imbalance: |current_ratio - ideal_ratio|
        // 5. Final fee: min(BASE_FEE + volatility_component + imbalance_component, MAX_FEE)

        // Calculate recent price volatility
        let (min_price, max_price, avg_price) =
            Self::get_price_range(observation_state, block_timestamp, VOLATILITY_WINDOW)?;
        // Handle case where no valid observations were found
        if min_price == 0 && max_price == 0 && avg_price == 0 {
            return Ok(base_fees);
        }

        let recent_price_volatility = if avg_price > 0 {
            max_price
                .saturating_sub(min_price)
                .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
                .and_then(|product| product.checked_div(avg_price))
                .unwrap_or(0)
        } else {
            0
        };

        let volatility_component_calculated = VOLATILITY_FACTOR
            .saturating_mul(recent_price_volatility as u64)
            .checked_div(FEE_RATE_DENOMINATOR_VALUE)
            .ok_or(GammaError::MathOverflow)?;

        // Calculate volatility component
        let volatility_component = std::cmp::min(
            volatility_component_calculated,
            MAX_FEE
                .checked_sub(base_fees)
                .ok_or(GammaError::MathOverflow)?,
        );
        
        // Calculate final dynamic fee
        let dynamic_fee = base_fees
            .checked_add(volatility_component)
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
        fee_type: FeeType,
        base_fees: u64,
    ) -> Result<u64> {
        match fee_type {
            FeeType::Volatility => Self::calculate_volatile_fee(
                block_timestamp,
                observation_state,
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

    /// Gets the price range within a specified time window
    ///
    /// # Arguments
    /// * `observation_state` - Historical price observations
    /// * `current_time` - The current timestamp
    /// * `window` - The time window to consider
    ///
    /// # Returns
    /// A tuple of (min_price, max_price, average_price) observed within the window
    fn get_price_range(
        observation_state: &ObservationState,
        current_time: u64,
        window: u64,
    ) -> Result<(u128, u128, u128)> {
        let mut min_price = u128::MAX;
        let mut max_price = 0u128;
        let mut total_price = 0u128;

        let mut descending_order_observations = observation_state
            .observations
            .iter()
            .enumerate()
            .filter(|(_, observation)| {
                observation.block_timestamp != 0
                    && observation.cumulative_token_0_price_x32 != 0
                    && observation.cumulative_token_1_price_x32 != 0
            })
            .map(|(index, observation)| ObservationWithIndex {
                index: index as u16,
                observation: *observation,
            })
            .collect::<Vec<_>>();

        descending_order_observations.sort_by(|a, b| {
            { b.observation.block_timestamp }.cmp(&{ a.observation.block_timestamp })
        });

        let mut count = 0;
        for observation_with_index in descending_order_observations {
            let is_in_observation_window = current_time
                .saturating_sub(observation_with_index.observation.block_timestamp)
                <= window;

            if !is_in_observation_window {
                // they are already in descending order of block timestamp.
                break;
            }
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

            let cumulative_token_0_price = observation_with_index
                .observation
                .cumulative_token_0_price_x32;

            let last_observation = &observation_state.observations[last_observation_index];
            let last_cumulative_token_0_price = last_observation.cumulative_token_0_price_x32;

            let time_delta = observation_with_index
                .observation
                .block_timestamp
                .saturating_sub(last_observation.block_timestamp)
                as u128;

            // Since we are using cumulative_token_price we are sure, that current_observation.cumulative_token_0_price > last_observation.cumulative_token_0_price
            let price = cumulative_token_0_price
                .checked_sub(last_cumulative_token_0_price)
                .ok_or(GammaError::MathOverflow)?
                .checked_div(time_delta)
                .ok_or(GammaError::MathOverflow)?;

            // change cumulative
            min_price = min_price.min(price);
            max_price = max_price.max(price);
            count += 1;

            total_price = total_price
                .checked_add(price)
                .ok_or(GammaError::MathOverflow)?;
        }

        if count == 0 {
            // If no valid observations found, return a default range
            // This could be (0, 0, 0) or another appropriate default
            return Ok((0, 0, 0));
        }

        // We are dividing  u128 by u128, we will lose precision here
        // This can be optimized.
        let price_avg = total_price
            .checked_div(count as u128)
            .ok_or(GammaError::MathOverflow)?;
        Ok((min_price, max_price, price_avg))
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
        fee_type: FeeType,
        base_fees: u64,
    ) -> Result<u128> {
        let dynamic_fee_rate = Self::calculate_dynamic_fee(
            block_timestamp,
            observation_state,
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
