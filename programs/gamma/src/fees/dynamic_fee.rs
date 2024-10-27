use super::{ceil_div, FEE_RATE_DENOMINATOR_VALUE};
use crate::{
    error::GammaError,
    states::{Observation, ObservationState},
};
use anchor_lang::prelude::*;
use rust_decimal::{Decimal, MathematicalOps};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;

//pub const FEE_RATE_DENOMINATOR_VALUE: u64 = 1_000_000;

// Volatility-based fee constants
pub const MAX_FEE_VOLATILITY: u64 = 10000; // 1% max fee
pub const VOLATILITY_WINDOW: u64 = 3600; // 1 hour window for volatility calculation

const MAX_FEE: u64 = 100000; // 10% max fee
const VOLATILITY_FACTOR: u64 = 30_000; // Adjust based on desired sensitivity

pub enum FeeType {
    Volatility,
}
pub struct DynamicFee {}

impl DynamicFee {
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

    pub fn calculate_volatile_fee(
        block_timestamp: u64,
        observation_state: &ObservationState,
        base_fees: u64,
    ) -> Result<u64> {
        // Get TWAPs for both tokens
        let (twap_token_0_price, twap_token_1_price) =
            Self::get_twap_prices(observation_state, block_timestamp, VOLATILITY_WINDOW)?;
    
        // Handle case where no valid observations were found
        if twap_token_0_price.is_zero() || twap_token_1_price.is_zero() {
            return Ok(base_fees);
        }
    
        // Calculate the logarithm of the price ratio
        let price_ratio = twap_token_0_price
            .checked_div(twap_token_1_price)
            .ok_or(GammaError::MathOverflow)?;
    
        // Calculate the absolute value of the logarithmic price change
        let log_price_ratio = price_ratio.ln();

        let abs_log_price_ratio = log_price_ratio.abs();
    
        // Scale the logarithmic value to match the fee rate denominator
        // Multiply by FEE_RATE_DENOMINATOR_VALUE to scale to the appropriate precision
        let scaled_log = abs_log_price_ratio
            .checked_mul(Decimal::from_u64(FEE_RATE_DENOMINATOR_VALUE).unwrap())
            .ok_or(GammaError::MathOverflow)?;
    
        // Convert scaled_log to u64
        let scaled_log_u64 = scaled_log
            .to_u64()
            .ok_or(GammaError::MathOverflow)?;
    
        // Calculate volatility component
        let volatility_component_calculated = VOLATILITY_FACTOR
            .saturating_mul(scaled_log_u64)
            .checked_div(FEE_RATE_DENOMINATOR_VALUE)
            .ok_or(GammaError::MathOverflow)?;
    
        let volatility_component = std::cmp::min(
            volatility_component_calculated,
            MAX_FEE.checked_sub(base_fees).ok_or(GammaError::MathOverflow)?,
        );
    
        // Calculate final dynamic fee
        let dynamic_fee = base_fees
            .checked_add(volatility_component)
            .ok_or(GammaError::MathOverflow)?;
    
        Ok(std::cmp::min(dynamic_fee, MAX_FEE))
    }    

    fn get_twap_prices(
        observation_state: &ObservationState,
        current_time: u64,
        window: u64,
    ) -> Result<(Decimal, Decimal)> {
        let window_start_time = current_time.saturating_sub(window);
    
        // Initialize variables to store observations at window start and end
        let mut observation_start: Option<&Observation> = None;
        let mut observation_end: Option<&Observation> = None;
    
        // Iterate over observations to find the ones closest to window_start_time and current_time
        for observation in observation_state.observations.iter() {
            if observation.block_timestamp == 0 {
                continue; // Skip uninitialized observations
            }
    
            // Find the observation closest to the window start time
            if observation.block_timestamp >= window_start_time {
                if observation_start.is_none()
                    || observation.block_timestamp < observation_start.unwrap().block_timestamp
                {
                    observation_start = Some(observation);
                }
            }
    
            // Find the latest observation up to the current time
            if observation.block_timestamp <= current_time {
                if observation_end.is_none()
                    || observation.block_timestamp > observation_end.unwrap().block_timestamp
                {
                    observation_end = Some(observation);
                }
            }
        }
    
        if observation_start.is_none() || observation_end.is_none() {
            // No valid observations found in the window
            return Ok((Decimal::from_u64(0).unwrap(), Decimal::from_u64(0).unwrap()));
        }
    
        let start_obs = observation_start.unwrap();
        let end_obs = observation_end.unwrap();
    
        let time_delta = end_obs
            .block_timestamp
            .saturating_sub(start_obs.block_timestamp);
        if time_delta == 0 {
            // Avoid division by zero
            return Ok((Decimal::from_u64(0).unwrap(), Decimal::from_u64(0).unwrap()));
        }
    
        // Convert time_delta to Decimal
        let time_delta_decimal = Decimal::from_u64(time_delta).ok_or(GammaError::MathOverflow)?;
    
        // Calculate TWAP for token 0
        let cumulative_price_delta_token_0 = end_obs
            .cumulative_token_0_price_x32
            .checked_sub(start_obs.cumulative_token_0_price_x32)
            .ok_or(GammaError::MathOverflow)?;
    
        let twap_token_0_price = Decimal::from_u128(cumulative_price_delta_token_0)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(time_delta_decimal)
            .ok_or(GammaError::MathOverflow)?;
    
        // Calculate TWAP for token 1
        let cumulative_price_delta_token_1 = end_obs
            .cumulative_token_1_price_x32
            .checked_sub(start_obs.cumulative_token_1_price_x32)
            .ok_or(GammaError::MathOverflow)?;
    
        let twap_token_1_price = Decimal::from_u128(cumulative_price_delta_token_1)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(time_delta_decimal)
            .ok_or(GammaError::MathOverflow)?;
    
        Ok((twap_token_0_price, twap_token_1_price))
    }
    
    /// Calculates the fee amount for a given input amount
    ///
    /// # Arguments
    /// * `amount` - The input amount
    /// * `pool_state` - The current state of the pool
    /// * `observation_state` - Historical price observations
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
