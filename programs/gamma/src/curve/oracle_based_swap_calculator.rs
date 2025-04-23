//! Oracle based swap calculations
use crate::curve::CurveCalculator;
use crate::error::GammaError;
use crate::fees::{ceil_div, DynamicFee, FeeType, FEE_RATE_DENOMINATOR_VALUE};
use crate::states::{AmmConfig, ObservationState, PoolState};
use crate::{curve::constant_product::ConstantProductCurve, fees::StaticFee};
use anchor_lang::prelude::*;

use super::{SwapResult, TradeDirection};
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
            .ok_or(GammaError::MathOverflow)?
            .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(GammaError::MathOverflow)?;

        // Max amount that can be swapped without reaching the acceptable price difference limit
        let price_difference_limit = FEE_RATE_DENOMINATOR_VALUE
            .checked_sub(pool_state.acceptable_price_difference.into())
            .ok_or(GammaError::MathOverflow)?;
        // We can swap with oracle price, P until we reach spot_price_at_acceptable_price_difference_limit Z
        // We want to calculate the spot_price_at_acceptable_price_difference_limit that is away from current oracle_price and not current spot_price.
        let spot_price_at_acceptable_price_difference_limit = oracle_price
            .checked_mul(price_difference_limit.into())
            .ok_or(GammaError::MathOverflow)?
            .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(GammaError::MathOverflow)?;

        // Max tradeable amount with price Oracle Price P before we reach spot_price_at_acceptable_price_difference_limit Z
        // Can we derived by the formula:
        // x_delta_max = (|(Z*X) - Y)| / (Z + P)
        let z_times_x = spot_price_at_acceptable_price_difference_limit
            .checked_mul(swap_source_amount)
            .ok_or(GammaError::MathOverflow)?;
        let y_scaled_by_d9 = swap_destination_amount
            .checked_mul(D9)
            .ok_or(GammaError::MathOverflow)?;

        // numerator = |(Z*X) - Y|
        let numerator = z_times_x.abs_diff(y_scaled_by_d9);
        // denominator = Z + P
        let denominator = oracle_price
            .checked_add(spot_price_at_acceptable_price_difference_limit)
            .ok_or(GammaError::MathOverflow)?;

        let max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference =
            numerator
                .checked_div(denominator)
                .ok_or(GammaError::MathOverflow)?;

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
            .ok_or(GammaError::MathOverflow)?
            .checked_div(oracle_price)
            .ok_or(GammaError::MathOverflow)?;

        Ok(rate_difference)
    }

    pub fn get_execution_oracle_price(
        oracle_price: u128,
        price_premium_for_swap_at_oracle_price: u128,
    ) -> Result<u128> {
        let oracle_price_premium = oracle_price
            .checked_mul(price_premium_for_swap_at_oracle_price)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(GammaError::MathOverflow)?;

        // Make our price slightly better than the oracle price.
        let execution_oracle_price = oracle_price
            .checked_add(oracle_price_premium)
            .ok_or(GammaError::MathOverflow)?;

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
            return CurveCalculator::swap_base_input(
                source_amount_to_be_swapped,
                swap_source_amount,
                swap_destination_amount,
                amm_config,
                pool_state,
                block_timestamp,
                observation_state,
                is_invoked_by_signed_segmenter,
            );
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
            .ok_or(GammaError::MathOverflow)?
            .checked_div(swap_source_amount)
            .ok_or(GammaError::MathOverflow)?;

        let oracle_price = match trade_direction {
            TradeDirection::OneForZero => pool_state.oracle_price_token_0_by_token_1,
            TradeDirection::ZeroForOne => D9_TIMES_D9
                .checked_div(pool_state.oracle_price_token_0_by_token_1)
                .ok_or(GammaError::MathOverflow)?,
        };

        let rate_difference =
            Self::get_spot_price_and_oracle_price_rate_difference(oracle_price, spot_price)?;
        if rate_difference > pool_state.acceptable_price_difference as u128 {
            // If the price difference between pool and oracle is too high, we will use the old calculator.
            return CurveCalculator::swap_base_input(
                source_amount_to_be_swapped,
                swap_source_amount,
                swap_destination_amount,
                amm_config,
                pool_state,
                block_timestamp,
                observation_state,
                is_invoked_by_signed_segmenter,
            );
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
            .ok_or(GammaError::MathOverflow)?;
        #[cfg(target_os = "solana")]
        msg!(
            "amount_to_be_swapped_at_oracle_price: {}, amount_to_be_swapped_with_invariant_curve: {}",
            amount_to_be_swapped_at_oracle_price,
            amount_to_be_swapped_with_invariant_curve
        );

        if amount_to_be_swapped_at_oracle_price == 0 {
            return CurveCalculator::swap_base_input(
                source_amount_to_be_swapped,
                swap_source_amount,
                swap_destination_amount,
                amm_config,
                pool_state,
                block_timestamp,
                observation_state,
                is_invoked_by_signed_segmenter,
            );
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
        .ok_or(GammaError::MathOverflow)?;

        let source_amount_to_be_swapped_after_fees = amount_to_be_swapped_at_oracle_price
            .checked_sub(trade_fees_for_oracle_swap)
            .ok_or(GammaError::MathOverflow)?;

        let execution_oracle_price = Self::get_execution_oracle_price(
            oracle_price,
            pool_state.price_premium_for_swap_at_oracle_price.into(),
        )?;

        // The price is Y/X, we have delta_x, so to find y, we need to do y = delta_x * price
        // Since price was scaled by D9, we need to scale down by D9
        let output_tokens = execution_oracle_price
            .checked_mul(source_amount_to_be_swapped_after_fees)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(D9)
            .ok_or(GammaError::MathOverflow)?;

        let new_swap_source_amount = swap_source_amount
            .checked_sub(amount_to_be_swapped_at_oracle_price)
            .ok_or(GammaError::MathOverflow)?;

        let new_swap_destination_amount = swap_destination_amount
            .checked_add(output_tokens)
            .ok_or(GammaError::MathOverflow)?;

        let trade_fees_for_invariant_curve = ceil_div(
            amount_to_be_swapped_with_invariant_curve.into(),
            dynamic_fee_rate.into(),
            FEE_RATE_DENOMINATOR_VALUE.into(),
        )
        .ok_or(GammaError::MathOverflow)?;

        let source_amount_after_fees = amount_to_be_swapped_with_invariant_curve
            .checked_sub(trade_fees_for_invariant_curve)
            .ok_or(GammaError::MathOverflow)?;
        let trade_fee_charged = trade_fees_for_invariant_curve
            .checked_add(trade_fees_for_oracle_swap)
            .ok_or(GammaError::MathOverflow)?;

        let trade_fee_rate = trade_fee_charged
            .checked_mul(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(GammaError::MathOverflow)?
            .checked_div(source_amount_to_be_swapped)
            .ok_or(GammaError::MathOverflow)?;

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
            .ok_or(GammaError::MathOverflow)?;

        let protocol_fee = StaticFee::protocol_fee(trade_fee_charged, amm_config.protocol_fee_rate)
            .ok_or(GammaError::InvalidFee)?;
        let fund_fee = StaticFee::fund_fee(trade_fee_charged, amm_config.fund_fee_rate)
            .ok_or(GammaError::InvalidFee)?;

        Ok(SwapResult {
            new_swap_source_amount: swap_source_amount
                .checked_add(source_amount_to_be_swapped)
                .ok_or(GammaError::MathOverflow)?,
            new_swap_destination_amount: swap_destination_amount
                .checked_sub(destination_amount_swapped)
                .ok_or(GammaError::MathOverflow)?,
            source_amount_swapped: source_amount_to_be_swapped,
            destination_amount_swapped,
            dynamic_fee: trade_fee_charged,
            protocol_fee,
            fund_fee,
            dynamic_fee_rate: trade_fee_rate as u64,
        })
    }
}

#[cfg(test)]
mod get_amount_to_be_swapped_at_oracle_price {
    use super::*;

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_basic_scenarios() {
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100_000; // 10% max swap at oracle
        pool_state.acceptable_price_difference = 50_000; // 5% acceptable difference

        // Test case 1: Normal swap within limits
        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            1_000,              // source_amount_to_be_swapped
            10_000,             // swap_source_amount
            10_000,             // swap_destination_amount
            1_000_000_000 * D9, // oracle_price (1:1 in D9)
            &pool_state,
        )
        .unwrap();

        assert!(result > 0, "Should allow normal swap within limits");
        assert!(result <= 1_000, "Should not exceed source amount");

        // Test case 2: Trying to swap more than max allowed
        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            2_000,              // source_amount_to_be_swapped (larger than max allowed)
            10_000,             // swap_source_amount
            10_000,             // swap_destination_amount
            1_000_000_000 * D9, // oracle_price
            &pool_state,
        )
        .unwrap();

        let expected_max = 10_000 * 100_000 / 1_000_000; // max_amount_swappable_at_oracle_price
        assert_eq!(
            result, expected_max,
            "Should be limited by max_amount_swappable_at_oracle_price"
        );
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_price_limits() {
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100_000;
        pool_state.acceptable_price_difference = 50_000; // 5% acceptable difference

        // Test case: When current pool state is at the acceptable price difference limit
        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            1_000,              // source_amount_to_be_swapped
            100_000,            // swap_source_amount
            95_000,             // swap_destination_amount (5% difference from 1:1)
            1_000_000_000 * D9, // oracle_price
            &pool_state,
        )
        .unwrap();

        assert!(result > 0, "Should allow swap at price difference limit");
        assert!(result <= 1_000, "Should not exceed source amount");
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_edge_cases() {
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100_000;
        pool_state.acceptable_price_difference = 50_000;

        // Test case 1: Minimum values
        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            1,                  // source_amount_to_be_swapped
            1,                  // swap_source_amount
            1,                  // swap_destination_amount
            1_000_000_000 * D9, // oracle_price
            &pool_state,
        )
        .unwrap();

        assert!(result <= 1, "Should handle minimum values correctly");

        // Test case 2: Zero source amount
        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            0,                  // source_amount_to_be_swapped
            1000,               // swap_source_amount
            1000,               // swap_destination_amount
            1_000_000_000 * D9, // oracle_price
            &pool_state,
        )
        .unwrap();

        assert_eq!(result, 0, "Should handle zero source amount");
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_pool_constraints() {
        // Test case 1: Very restrictive pool settings
        let mut restrictive_pool = PoolState::default();
        restrictive_pool.max_amount_swappable_at_oracle_price = 1_000; // Only 0.1% at oracle
        restrictive_pool.acceptable_price_difference = 10_000; // Only 1% difference allowed

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            10_000,        // source_amount_to_be_swapped
            100_000,       // swap_source_amount
            100_000,       // swap_destination_amount
            1_000_000_000, // oracle_price
            &restrictive_pool,
        )
        .unwrap();

        let expected_max = 100_000 * 1_000 / 1_000_000; // max_amount_swappable_at_oracle_price
        assert_eq!(
            result, expected_max,
            "Should respect restrictive pool settings"
        );

        // Test case 2: Very permissive pool settings
        let mut permissive_pool = PoolState::default();
        permissive_pool.max_amount_swappable_at_oracle_price = 900_000; // 90% at oracle
        permissive_pool.acceptable_price_difference = 100_000; // 10% difference allowed

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            1_000,              // source_amount_to_be_swapped
            10_000,             // swap_source_amount
            10_000,             // swap_destination_amount
            1_000_000_000 * D9, // oracle_price
            &permissive_pool,
        )
        .unwrap();

        assert_eq!(
            result, 1_000,
            "Should allow full swap with permissive settings"
        );
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_different_prices() {
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100_000;
        pool_state.acceptable_price_difference = 50_000;

        // Test with different oracle prices
        let prices = vec![
            500_000_000 * D9,   // 0.5
            1_000_000_000 * D9, // 1.0
            2_000_000_000 * D9, // 2.0
        ];

        for oracle_price in prices {
            let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
                1_000,        // source_amount_to_be_swapped
                10_000,       // swap_source_amount
                10_000,       // swap_destination_amount
                oracle_price, // varying oracle price
                &pool_state,
            )
            .unwrap();

            assert!(result > 0, "Should handle different oracle prices");
            assert!(result <= 1_000, "Should not exceed source amount");
        }
    }
}

#[cfg(test)]
mod get_spot_price_and_oracle_price_rate_difference_tests {
    use super::*;
    #[test]
    fn test_basic_scenarios() {
        // Test case 1: Spot price higher than oracle price by 5%
        let oracle_price = 1_000_000_000; // 1.0 in D9 format
        let spot_price = 1_050_000_000; // 1.05 in D9 format

        let result = OracleBasedSwapCalculator::get_spot_price_and_oracle_price_rate_difference(
            oracle_price,
            spot_price,
        )
        .unwrap();

        // Expected: 5% of FEE_RATE_DENOMINATOR_VALUE (1_000_000)
        assert_eq!(result, 50_000, "Should calculate 5% difference correctly");

        // Test case 2: Spot price equals oracle price
        let result = OracleBasedSwapCalculator::get_spot_price_and_oracle_price_rate_difference(
            oracle_price,
            oracle_price,
        )
        .unwrap();

        assert_eq!(result, 0, "Should return 0 when prices are equal");

        // Test case 3: Spot price lower than oracle price by 3%
        let spot_price = 970_000_000; // 0.97 in D9 format

        let result = OracleBasedSwapCalculator::get_spot_price_and_oracle_price_rate_difference(
            oracle_price,
            spot_price,
        )
        .unwrap();

        assert_eq!(result, 30_000, "Should calculate 3% difference correctly");
    }

    #[test]
    fn test_different_price_scales() {
        // Test with different price scales to ensure correct percentage calculation
        let test_cases = vec![
            // (oracle_price, spot_price, expected_difference)
            (1_000_000_000, 1_100_000_000, 100_000), // 10% difference
            (500_000_000, 550_000_000, 100_000),     // 10% difference at different scale
            (2_000_000_000, 2_200_000_000, 100_000), // 10% difference at larger scale
        ];

        for (oracle_price, spot_price, expected) in test_cases {
            let result =
                OracleBasedSwapCalculator::get_spot_price_and_oracle_price_rate_difference(
                    oracle_price,
                    spot_price,
                )
                .unwrap();

            assert_eq!(
                result, expected,
                "Failed for oracle_price: {}, spot_price: {}, expected: {}, got: {}",
                oracle_price, spot_price, expected, result
            );
        }
    }
}

#[cfg(test)]
mod get_execution_oracle_price_tests {
    use super::*;

    #[test]
    fn test_basic_scenarios() {
        // Test case 1: Standard case with price premium
        let oracle_price = 1_000_000_000; // 1.0 in D9 format
        let price_premium_for_swap_at_oracle_price = 1000; // 0.1% premium

        let result = OracleBasedSwapCalculator::get_execution_oracle_price(
            oracle_price,
            price_premium_for_swap_at_oracle_price,
        )
        .unwrap();

        // Expected: oracle_price + (oracle_price * price_premium / FEE_RATE_DENOMINATOR_VALUE)
        // 1_000_000_000 + (1_000_000_000 * 1000 / 1_000_000) = 1_000_000_000 + 1_000_000 = 1_001_000_000
        assert_eq!(result, 1_001_000_000);

        // Test case 2: Zero premium
        let price_premium_for_swap_at_oracle_price = 0;

        let result = OracleBasedSwapCalculator::get_execution_oracle_price(
            oracle_price,
            price_premium_for_swap_at_oracle_price,
        )
        .unwrap();

        // Should remain the same as the oracle price
        assert_eq!(result, oracle_price);

        // Test case 3: High premium
        let price_premium_for_swap_at_oracle_price = 100000; // 10% premium

        let result = OracleBasedSwapCalculator::get_execution_oracle_price(
            oracle_price,
            price_premium_for_swap_at_oracle_price,
        )
        .unwrap();

        // Expected: 1_000_000_000 + (1_000_000_000 * 100000 / 1_000_000) = 1_000_000_000 + 100_000_000 = 1_100_000_000
        assert_eq!(result, 1_100_000_000);
    }
}
