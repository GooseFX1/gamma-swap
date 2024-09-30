use super::{ceil_div, FEE_RATE_DENOMINATOR_VALUE};
use crate::{
    curve::ConstantProductCurve,
    states::{Observation, ObservationState, PoolState, OBSERVATION_NUM, Q32},
};
use anchor_lang::prelude::*;

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
    Rebalancing,
    Combined,
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
        vault_0: u64,
        vault_1: u64,
        base_fees: u64,
    ) -> Result<u64> {
        // 1. Price volatility: (max_price - min_price) / avg_price
        // 2. Volatility component: min(VOLATILITY_FACTOR * volatility, MAX_FEE - BASE_FEE)
        // 3. Liquidity imbalance: |current_ratio - ideal_ratio|
        // 4. Imbalance component: IMBALANCE_FACTOR * imbalance / FEE_RATE_DENOMINATOR_VALUE
        // 5. Final fee: min(BASE_FEE + volatility_component + imbalance_component, MAX_FEE)

        // Calculate recent price volatility
        let (min_price, max_price, avg_price) =
            Self::get_price_range(observation_state, block_timestamp, VOLATILITY_WINDOW);
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
        
        // Calculate volatility component
        let volatility_component = std::cmp::min(
            VOLATILITY_FACTOR.saturating_mul(recent_price_volatility as u64)
                / FEE_RATE_DENOMINATOR_VALUE,
            MAX_FEE.saturating_sub(base_fees),
        );
        
        // Calculate liquidity imbalance component
        let total_liquidity = vault_0 as u128 + vault_1 as u128;
        let current_ratio = if total_liquidity > 0 {
            (vault_0 as u128)
                .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
                .and_then(|product| product.checked_div(total_liquidity))
                .unwrap_or(0)
        } else {
            0
        };

        let ideal_ratio = FEE_RATE_DENOMINATOR_VALUE as u128 / 2;
        
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
            .saturating_add(volatility_component)
            .saturating_add(liquidity_imbalance_component);
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
        pool_state: &PoolState,
        observation_state: &ObservationState,
        vault_0: u64,
        vault_1: u64,
        fee_type: FeeType,
        base_fees: u64,
    ) -> u64 {
        match fee_type {
            FeeType::Volatility => Self::calculate_volatile_fee_v2(
                block_timestamp,
                observation_state,
                vault_0,
                vault_1,
                base_fees,
            )
            .unwrap(),
            FeeType::Rebalancing => Self::calculate_rebalancing_fee(pool_state, vault_0, vault_1),
            FeeType::Combined => {
                let rebalancing_fee = Self::calculate_rebalancing_fee(pool_state, vault_0, vault_1);
                let volatility_fee = Self::calculate_volatile_fee(
                    block_timestamp,
                    observation_state,
                    vault_0,
                    vault_1,
                    base_fees,
                )
                .unwrap();
                std::cmp::max(rebalancing_fee, volatility_fee)
            }
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
            Self::get_price_range(observation_state, block_timestamp, VOLATILITY_WINDOW);
        let volatility = if price_b > price_a {
            (price_b - price_a)
                .checked_div(price_a)
                .unwrap()
                .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
                .unwrap()
        } else {
            (price_a - price_b)
                .checked_div(price_b)
                .unwrap()
                .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
                .unwrap()
        };

        let dynamic_fee = volatility
            .checked_div(100)
            .unwrap()
            .checked_add(base_fees as u128)
            .unwrap(); // Increase fee by 1 bp for each 1% of volatility
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
    ) -> (u128, u128, u128) {
        let mut min_price = u128::MAX;
        let mut max_price = 0u128;
        let mut total_price = 0u128;

        let mut descending_order_observations = observation_state
            .observations
            .iter()
            .filter(|x| {
                x.block_timestamp != 0
                    && x.cumulative_token_0_price_x32 != 0
                    && x.cumulative_token_1_price_x32 != 0
            })
            .enumerate()
            .map(|(index, observation)| ObservationWithIndex {
                index: index.try_into().unwrap(),
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

            let price = cumulative_token_0_price
                .checked_sub(last_cumulative_token_0_price)
                .unwrap()
                .checked_div(time_delta)
                .unwrap();

            // change cumulative
            min_price = min_price.min(price);
            max_price = max_price.max(price);
            count += 1;
            // checked add?
            total_price += price;
        }

        if count == 0 {
            // If no valid observations found, return a default range
            // This could be (0, 0, 0) or another appropriate default
            return (0, 0, 0);
        }

        // We are dividing  u128 by u128, we will lose precision here
        // This can be optimized.
        (min_price, max_price, total_price / count as u128)
    }

    /// Calculates a fee based on pool utilization and price deviation
    ///
    /// # Arguments
    /// * `pool_state` - The current state of the pool
    /// * `vault_0` - Amount of token 0 in the vault
    /// * `vault_1` - Amount of token 1 in the vault
    ///
    /// # Returns
    /// A fee rate as a u64, where 10_000_000 represents 10%
    fn calculate_rebalancing_fee(pool_state: &PoolState, vault_0: u64, vault_1: u64) -> u64 {
        let (token_0_amount, token_1_amount) =
            pool_state.vault_amount_without_fee(vault_0, vault_1);
        let (price_0, _price_1) = pool_state.token_price_x32(token_0_amount, token_1_amount);

        // 1. Utilization = max(token_0, token_1) / total_liquidity * FEE_RATE_DENOMINATOR_VALUE
        // 2. Price deviation = |current_price - ideal_price| / ideal_price * FEE_RATE_DENOMINATOR_VALUE
        // 3. Base fee calculation:
        //    - If utilization <= 50%: linear interpolation between MIN_FEE and MID_FEE
        //    - If 50% < utilization <= 85%: linear interpolation between MID_FEE and OUT_FEE
        //    - If utilization > 85%: linear interpolation between OUT_FEE and MAX_FEE
        // 4. Adjusted fee = base_fee + (price_deviation * base_fee / FEE_RATE_DENOMINATOR_VALUE)

        // Calculate utilization
        let total_liquidity = token_0_amount.checked_add(token_1_amount).unwrap() as u128;
        let max_token = token_0_amount.max(token_1_amount) as u128;

        let utilization = max_token
            .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
            .unwrap()
            .checked_div(total_liquidity)
            .unwrap();

        assert!(utilization < u64::MAX as u128);
        let utilization = utilization as u64;

        // Calculate price deviation
        let complete_swap = ConstantProductCurve::swap_base_input_without_fees(
            token_1_amount as u128,
            token_1_amount as u128,
            token_0_amount as u128,
        );
        // we multiply by Q32 to make sure all both the prices have decimal bits.
        let ideal_price = complete_swap
            .checked_mul(Q32)
            .unwrap()
            .checked_div(token_1_amount as u128)
            .unwrap();

        let price_deviation_numerator = price_0.abs_diff(ideal_price);

        // Calculate fee based on utilization and price deviation
        let fee = if utilization <= 500_000 {
            // 50%
            MIN_FEE_REBALANCE + (MID_FEE_REBALANCE - MIN_FEE_REBALANCE) * utilization / 500_000
        } else if utilization <= 850_000 {
            // 85%
            MID_FEE_REBALANCE
                + (OUT_FEE_REBALANCE - MID_FEE_REBALANCE) * (utilization - 500_000) / 350_000
        } else {
            OUT_FEE_REBALANCE
                + (MAX_FEE_REBALANCE - OUT_FEE_REBALANCE) * (utilization - 850_000) / 150_000
        } as u128;

        // Adjust fee based on price deviation
        let additional_fee = price_deviation_numerator
            .checked_mul(fee)
            .unwrap()
            .checked_div(ideal_price)
            .unwrap();
        let adjusted_fee = fee as u128 + additional_fee;

        assert!(adjusted_fee < u64::MAX as u128);
        let adjusted_fee = adjusted_fee as u64;

        adjusted_fee.min(MAX_FEE_REBALANCE)
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
        pool_state: &PoolState,
        observation_state: &ObservationState,
        vault_0: u64,
        vault_1: u64,
        fee_type: FeeType,
        base_fees: u64,
    ) -> Option<u128> {
        let dynamic_fee_rate = Self::calculate_dynamic_fee(
            block_timestamp,
            pool_state,
            observation_state,
            vault_0,
            vault_1,
            fee_type,
            base_fees,
        );

        ceil_div(
            amount,
            u128::from(dynamic_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
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
        pool_state: &PoolState,
        observation_state: &ObservationState,
        vault_0: u64,
        vault_1: u64,
        fee_type: FeeType,
        base_fees: u64,
    ) -> Option<u128> {
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
            pool_state,
            observation_state,
            vault_0,
            vault_1,
            fee_type,
            base_fees,
        );
        if dynamic_fee_rate == 0 {
            Some(post_fee_amount)
        } else {
            let numerator = post_fee_amount.checked_mul(u128::from(FEE_RATE_DENOMINATOR_VALUE))?;
            let denominator =
                u128::from(FEE_RATE_DENOMINATOR_VALUE).checked_sub(u128::from(dynamic_fee_rate))?;

            numerator
                .checked_add(denominator)?
                .checked_sub(1)?
                .checked_div(denominator)
        }
    }
}
