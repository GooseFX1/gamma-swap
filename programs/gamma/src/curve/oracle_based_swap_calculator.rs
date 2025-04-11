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
        // If swap is happening from x->y price is y/x
        // If swap is happening from y->x Price is x/y
        spot_price: u128,
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
        let spot_price_at_acceptable_price_difference_limit = spot_price
            .checked_mul(price_difference_limit.into())
            .ok_or(GammaError::MathOverflow)?
            .checked_div(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(GammaError::MathOverflow)?;
        msg!(
            "spot_price_at_acceptable_price_difference_limit: {}",
            spot_price_at_acceptable_price_difference_limit
        );

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
        if difference > amm_config.max_oracle_price_update_time_diff
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
            spot_price,
        )?;
        let amount_to_be_swapped_with_invariant_curve = source_amount_to_be_swapped
            .checked_sub(amount_to_be_swapped_at_oracle_price)
            .ok_or(GammaError::MathOverflow)?;
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
            .checked_sub(source_amount_to_be_swapped_after_fees)
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

        let destination_amount_swapped_with_curve_calculator =
            ConstantProductCurve::swap_base_input_without_fees(
                source_amount_after_fees,
                new_swap_source_amount,
                new_swap_destination_amount,
            )?;

        let trade_fee_charged = trade_fees_for_invariant_curve
            .checked_add(trade_fees_for_oracle_swap)
            .ok_or(GammaError::MathOverflow)?;

        let trade_fee_rate = trade_fee_charged
            .checked_mul(FEE_RATE_DENOMINATOR_VALUE.into())
            .ok_or(GammaError::MathOverflow)?
            .checked_div(source_amount_to_be_swapped)
            .ok_or(GammaError::MathOverflow)?;

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
mod tests {
    use super::*;
    use crate::states::{AmmConfig, ObservationState};

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price() {
        // Test case 1: Standard case where oracle price and spot price are close
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100000; // 10% of vault amount
        pool_state.acceptable_price_difference = 50000; // 5% difference

        // Let's calculate all values step by step
        let source_amount_to_be_swapped: u128 = 100;
        let swap_source_amount: u128 = 1000;
        let swap_destination_amount: u128 = 500; // Lower than swap_source_amount * spot_price
        let oracle_price: u128 = 1_000_000_000; // 1:1 price in D9 format
        let spot_price: u128 = 1_050_000_000; // 1.05:1 price (5% above oracle)

        // Step 1: Calculate max_amount_swappable_at_oracle_price
        let max_amount_swappable_at_oracle_price = swap_source_amount
            * pool_state.max_amount_swappable_at_oracle_price as u128
            / FEE_RATE_DENOMINATOR_VALUE as u128; // 1000 * 100000 / 1000000 = 100

        // Step 2: Calculate price_difference_limit
        let price_difference_limit =
            FEE_RATE_DENOMINATOR_VALUE as u128 - pool_state.acceptable_price_difference as u128; // 1000000 - 50000 = 950000

        // Step 3: Calculate spot_price_at_acceptable_price_difference_limit
        let spot_price_at_acceptable_price_difference_limit =
            spot_price * price_difference_limit / FEE_RATE_DENOMINATOR_VALUE as u128;
        // 1050000000 * 950000 / 1000000 = 997,500,000

        // Step 4: Calculate z_times_x
        let z_times_x = spot_price_at_acceptable_price_difference_limit * swap_source_amount;
        // 997,500,000 * 1000 = 997,500,000,000

        // Verify z_times_x > swap_destination_amount
        assert!(
            z_times_x > swap_destination_amount,
            "Test setup incorrect: z_times_x ({}) must be > swap_destination_amount ({})",
            z_times_x,
            swap_destination_amount
        );

        // Step 5: Calculate numerator = z_times_x - swap_destination_amount
        let numerator = z_times_x - swap_destination_amount;
        // 997,500,000,000 - 500 = 997,499,999,500

        // Step 6: Calculate denominator = oracle_price + spot_price_at_acceptable_price_difference_limit
        let denominator = oracle_price + spot_price_at_acceptable_price_difference_limit;
        // 1,000,000,000 + 997,500,000 = 1,997,500,000

        // Step 7: Calculate max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference
        let max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference =
            numerator / denominator;
        // 997,499,999,500 / 1,997,500,000 ≈ 499

        // Step 8: Calculate max_swap_at_oracle_price
        let max_swap_at_oracle_price = std::cmp::min(
            max_amount_swappable_at_oracle_price,
            max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference,
        );
        // min(100, 499) = 100

        // Step 9: Final result
        let expected_result = std::cmp::min(max_swap_at_oracle_price, source_amount_to_be_swapped);
        // min(100, 100) = 100

        // Print all intermediate calculations for debugging
        println!(
            "max_amount_swappable_at_oracle_price: {}",
            max_amount_swappable_at_oracle_price
        );
        println!("price_difference_limit: {}", price_difference_limit);
        println!(
            "spot_price_at_acceptable_price_difference_limit: {}",
            spot_price_at_acceptable_price_difference_limit
        );
        println!("z_times_x: {}", z_times_x);
        println!("numerator: {}", numerator);
        println!("denominator: {}", denominator);
        println!(
            "max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference: {}",
            max_amount_swappable_at_oracle_price_without_reaching_acceptable_price_difference
        );
        println!("max_swap_at_oracle_price: {}", max_swap_at_oracle_price);
        println!("expected_result: {}", expected_result);

        // Now call the actual function
        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        println!("actual_result: {}", result);

        // Compare the expected and actual results
        assert_eq!(
            result, expected_result,
            "Expected result {} but got {}",
            expected_result, result
        );

        // Test case 2: When z_times_x < swap_destination_amount
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 1000; // Reduced to 0.1% of vault amount
        pool_state.acceptable_price_difference = 999900; // 99.99% difference - will cause z_times_x to be extremely small

        // Create a scenario where z_times_x is definitely < swap_destination_amount
        let source_amount_to_be_swapped: u128 = 10; // Reduced to match expected behavior
        let swap_source_amount: u128 = 100; // Keep small to make z_times_x smaller
        let swap_destination_amount: u128 = 1_000_000_000; // Much larger value
        let oracle_price: u128 = 1_000_000_000; // 1:1 price
        let spot_price: u128 = 1_000_000_000; // 1:1 price

        // Calculate z_times_x for verification
        let price_difference_limit =
            FEE_RATE_DENOMINATOR_VALUE as u128 - pool_state.acceptable_price_difference as u128; // Only 0.01% left (100)
        let spot_price_at_acceptable_price_difference_limit =
            spot_price * price_difference_limit / FEE_RATE_DENOMINATOR_VALUE as u128;
        let z_times_x = spot_price_at_acceptable_price_difference_limit * swap_source_amount;

        println!(
            "Test case 2 - price_difference_limit: {}",
            price_difference_limit
        );
        println!(
            "Test case 2 - spot_price_at_acceptable_price_difference_limit: {}",
            spot_price_at_acceptable_price_difference_limit
        );
        println!("Test case 2 - z_times_x: {}", z_times_x);
        println!(
            "Test case 2 - swap_destination_amount: {}",
            swap_destination_amount
        );

        // Make sure swap_destination_amount is much larger than z_times_x
        assert!(
            z_times_x < swap_destination_amount,
            "Test setup incorrect: z_times_x ({}) must be < swap_destination_amount ({})",
            z_times_x,
            swap_destination_amount
        );

        // When z_times_x < swap_destination_amount, it should return 0
        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        assert_eq!(
            result, 0,
            "When z_times_x < swap_destination_amount, result should be 0"
        );

        // Test case 3: When the max_amount_swappable_at_oracle_price is the limiting factor
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 5000; // 0.5% of vault amount
        pool_state.acceptable_price_difference = 50000; // 5% difference

        let source_amount_to_be_swapped: u128 = 1000;
        let swap_source_amount: u128 = 1000;
        let swap_destination_amount: u128 = 500; // Adjusted to ensure calculations work
        let oracle_price: u128 = 1_000_000_000; // 1:1 price
        let spot_price: u128 = 1_030_000_000; // 1.03:1 price (3% above oracle)

        // Calculate the expected value
        let expected = swap_source_amount * pool_state.max_amount_swappable_at_oracle_price as u128
            / FEE_RATE_DENOMINATOR_VALUE as u128;
        // 1000 * 5000 / 1000000 = 5

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        // Should be limited by max_amount_swappable_at_oracle_price (5000/1_000_000 * 1000 = 5)
        assert_eq!(
            result, expected,
            "Expected result to be limited by max_amount_swappable_at_oracle_price"
        );
    }

    #[test]
    fn test_get_spot_price_and_oracle_price_rate_difference() {
        // Test case 1: Spot price is higher than oracle price
        let oracle_price = 1_000_000_000; // 1:1 price in D9 format
        let spot_price = 1_050_000_000; // 1.05:1 price (5% above oracle)

        let result = OracleBasedSwapCalculator::get_spot_price_and_oracle_price_rate_difference(
            oracle_price,
            spot_price,
        )
        .unwrap();

        // Difference should be 5% of FEE_RATE_DENOMINATOR_VALUE (1_000_000)
        assert_eq!(result, 50000);

        // Test case 2: Spot price equals oracle price
        let oracle_price = 1_000_000_000;
        let spot_price = 1_000_000_000;

        let result = OracleBasedSwapCalculator::get_spot_price_and_oracle_price_rate_difference(
            oracle_price,
            spot_price,
        )
        .unwrap();

        // No difference
        assert_eq!(result, 0);

        // Test case 3: Spot price is lower than oracle price (should not happen in normal operation)
        // But the function should handle it appropriately with error
        let oracle_price = 1_050_000_000;
        let spot_price = 1_000_000_000;

        // This should return an error as spot_price < oracle_price
        let result = OracleBasedSwapCalculator::get_spot_price_and_oracle_price_rate_difference(
            oracle_price,
            spot_price,
        );

        // In the current implementation this would return an error due to underflow
        assert!(result.is_err());
    }

    #[test]
    fn test_get_execution_oracle_price() {
        // Test case 1: Standard case with price premium
        let oracle_price = 1_000_000_000; // 1:1 price in D9 format
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

    #[test]
    fn test_swap_base_input() {
        // Create a mock AmmConfig
        let amm_config = AmmConfig {
            max_oracle_price_update_time_diff: 60, // 60 seconds
            trade_fee_rate: 3000,                  // 0.3%
            protocol_fee_rate: 100000,             // 10% of fee goes to protocol
            fund_fee_rate: 100000,                 // 10% of fee goes to fund
            ..AmmConfig::default()
        };

        // Create a mock PoolState
        let mut pool_state = PoolState::default();
        pool_state.oracle_price_token_0_by_token_1 = 1_000_000_000; // 1:1 price in D9 format
        pool_state.oracle_price_updated_at = 1000; // Some timestamp
        pool_state.acceptable_price_difference = 50000; // 5% difference allowed
        pool_state.max_amount_swappable_at_oracle_price = 100000; // 10% of pool can be swapped at oracle price
        pool_state.min_trade_rate_at_oracle_price = 1000; // 0.1% min fee for oracle swaps
        pool_state.price_premium_for_swap_at_oracle_price = 1000; // 0.1% premium
        pool_state.token_0_vault_amount = 1000000; // Vault amounts
        pool_state.token_1_vault_amount = 1000000;

        // Create a mock ObservationState
        let observation_state = ObservationState::default();

        // Test case 1: Oracle is recent and price difference is acceptable
        // Use a larger amount to ensure fees are non-zero
        let source_amount_to_be_swapped = 100000; // Increased from 10000
        let swap_source_amount = 1000000;
        let swap_destination_amount = 1000000;
        let block_timestamp = 1050; // Just a bit after oracle update
        let is_invoked_by_signed_segmenter = false;

        let result = OracleBasedSwapCalculator::swap_base_input(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            &amm_config,
            &pool_state,
            block_timestamp,
            &observation_state,
            is_invoked_by_signed_segmenter,
        );

        // Check that we got a valid result
        assert!(result.is_ok());
        let swap_result = result.unwrap();

        // Check that result values make sense
        assert_eq!(
            swap_result.source_amount_swapped,
            source_amount_to_be_swapped
        );
        assert!(swap_result.destination_amount_swapped > 0);
        assert!(swap_result.dynamic_fee > 0);

        // Check the specific fee values to debug
        println!("dynamic_fee: {}", swap_result.dynamic_fee);
        println!("protocol_fee: {}", swap_result.protocol_fee);
        println!("fund_fee: {}", swap_result.fund_fee);

        // Now we're using higher percentages for protocol_fee_rate and fund_fee_rate
        // And a larger source amount, so these should be positive
        assert!(
            swap_result.protocol_fee > 0,
            "Protocol fee should be non-zero"
        );
        assert!(swap_result.fund_fee > 0, "Fund fee should be non-zero");

        // Test case 2: Oracle is outdated - should fall back to CurveCalculator
        let outdated_block_timestamp = 1070; // More than 60 seconds after oracle update

        let result = OracleBasedSwapCalculator::swap_base_input(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            &amm_config,
            &pool_state,
            outdated_block_timestamp,
            &observation_state,
            is_invoked_by_signed_segmenter,
        );

        // Should still succeed using the fallback calculator
        assert!(result.is_ok());

        // Test case 3: Price difference is too high - should fall back to CurveCalculator
        let mut high_diff_pool_state = pool_state.clone();
        high_diff_pool_state.oracle_price_token_0_by_token_1 = 800_000_000; // 20% lower than spot price

        let result = OracleBasedSwapCalculator::swap_base_input(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            &amm_config,
            &high_diff_pool_state,
            block_timestamp,
            &observation_state,
            is_invoked_by_signed_segmenter,
        );

        // Should still succeed using the fallback calculator
        assert!(result.is_ok());

        // Test case 4: No amount can be swapped at oracle price - should fall back to CurveCalculator
        let mut zero_oracle_pool_state = pool_state.clone();
        zero_oracle_pool_state.max_amount_swappable_at_oracle_price = 0; // 0% can be swapped at oracle price

        let result = OracleBasedSwapCalculator::swap_base_input(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            &amm_config,
            &zero_oracle_pool_state,
            block_timestamp,
            &observation_state,
            is_invoked_by_signed_segmenter,
        );

        // Should still succeed using the fallback calculator
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_price_scenarios() {
        // Test case 1: When spot price is significantly higher than oracle price
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100000; // 10% of vault amount
        pool_state.acceptable_price_difference = 50000; // 5% difference allowed

        let source_amount_to_be_swapped: u128 = 1_000_000;
        let swap_source_amount: u128 = 10_000_000;
        let swap_destination_amount: u128 = 100_000; // Significantly reduced to ensure z_times_x > swap_destination_amount
        let oracle_price: u128 = 1_000_000_000;
        let spot_price: u128 = 1_040_000_000; // 4% higher than oracle

        // Calculate z_times_x for verification
        let price_difference_limit =
            FEE_RATE_DENOMINATOR_VALUE as u128 - pool_state.acceptable_price_difference as u128;
        let spot_price_at_acceptable_price_difference_limit =
            spot_price * price_difference_limit / FEE_RATE_DENOMINATOR_VALUE as u128;
        let z_times_x = spot_price_at_acceptable_price_difference_limit * swap_source_amount;

        println!("Test case 1 - z_times_x: {}", z_times_x);
        println!(
            "Test case 1 - swap_destination_amount: {}",
            swap_destination_amount
        );
        println!(
            "Test case 1 - price_difference_limit: {}",
            price_difference_limit
        );
        println!(
            "Test case 1 - spot_price_at_acceptable_price_difference_limit: {}",
            spot_price_at_acceptable_price_difference_limit
        );

        // Ensure z_times_x > swap_destination_amount for the test to work
        assert!(
            z_times_x > swap_destination_amount,
            "Test setup incorrect: z_times_x must be > swap_destination_amount"
        );

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        // Should allow swap as price difference is within acceptable range
        assert!(
            result > 0,
            "Should allow swap when price difference is within acceptable range"
        );

        // Test case 2: When spot price is at the exact acceptable difference limit
        let spot_price: u128 = 1_050_000_000; // 5% higher than oracle
        let swap_destination_amount: u128 = 100_000; // Keep the same reduced amount

        // Recalculate z_times_x for new spot price
        let spot_price_at_acceptable_price_difference_limit =
            spot_price * price_difference_limit / FEE_RATE_DENOMINATOR_VALUE as u128;
        let z_times_x = spot_price_at_acceptable_price_difference_limit * swap_source_amount;

        println!("Test case 2 - z_times_x: {}", z_times_x);
        println!(
            "Test case 2 - swap_destination_amount: {}",
            swap_destination_amount
        );
        println!("Test case 2 - spot_price: {}", spot_price);
        println!(
            "Test case 2 - price_difference_limit: {}",
            price_difference_limit
        );
        println!(
            "Test case 2 - spot_price_at_acceptable_price_difference_limit: {}",
            spot_price_at_acceptable_price_difference_limit
        );

        // Ensure z_times_x > swap_destination_amount for the test to work
        assert!(
            z_times_x > swap_destination_amount,
            "Test setup incorrect: z_times_x must be > swap_destination_amount"
        );

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        // Calculate max_swappable for verification
        let max_swappable = swap_source_amount
            * pool_state.max_amount_swappable_at_oracle_price as u128
            / FEE_RATE_DENOMINATOR_VALUE as u128;
        println!("Test case 2 - result: {}", result);
        println!("Test case 2 - max_swappable: {}", max_swappable);
        println!(
            "Test case 2 - source_amount_to_be_swapped: {}",
            source_amount_to_be_swapped
        );

        // Should still allow swap but with reduced amount
        assert!(
            result > 0,
            "Should allow swap at acceptable difference limit"
        );
        assert!(
            result < source_amount_to_be_swapped,
            "Swap amount should be reduced at limit"
        );

        // Test case 3: When spot price exceeds acceptable difference
        let spot_price: u128 = 1_060_000_000; // 6% higher than oracle
        let swap_destination_amount: u128 = 100_000; // Keep the same reduced amount

        // Calculate rate difference for verification
        let difference_in_oracle_price = spot_price.checked_sub(oracle_price).unwrap();
        let rate_difference = difference_in_oracle_price
            .checked_mul(FEE_RATE_DENOMINATOR_VALUE as u128)
            .unwrap()
            .checked_div(oracle_price)
            .unwrap();

        let acceptable_price_difference = pool_state.acceptable_price_difference as u128;

        println!("Test case 3 - spot_price: {}", spot_price);
        println!("Test case 3 - oracle_price: {}", oracle_price);
        println!("Test case 3 - rate_difference: {}", rate_difference);
        println!(
            "Test case 3 - acceptable_price_difference: {}",
            acceptable_price_difference
        );

        // Verify our test setup is correct
        assert!(rate_difference > acceptable_price_difference,
            "Test setup incorrect: rate_difference ({}) must exceed acceptable_price_difference ({})",
            rate_difference, acceptable_price_difference);

        // Recalculate z_times_x for new spot price to verify the condition
        let spot_price_at_acceptable_price_difference_limit =
            spot_price * price_difference_limit / FEE_RATE_DENOMINATOR_VALUE as u128;
        let z_times_x = spot_price_at_acceptable_price_difference_limit * swap_source_amount;

        println!("Test case 3 - z_times_x: {}", z_times_x);
        println!(
            "Test case 3 - swap_destination_amount: {}",
            swap_destination_amount
        );

        // Ensure z_times_x > swap_destination_amount for the test to work
        assert!(
            z_times_x > swap_destination_amount,
            "Test setup incorrect: z_times_x must be > swap_destination_amount"
        );

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        // Should return 0 as price difference exceeds acceptable range
        assert_eq!(
            result, 0,
            "Should return 0 when price difference ({}) exceeds acceptable_price_difference ({})",
            rate_difference, acceptable_price_difference
        );
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_pool_constraints() {
        let mut pool_state = PoolState::default();

        // Test case 1: Very small max_amount_swappable_at_oracle_price
        pool_state.max_amount_swappable_at_oracle_price = 1000; // 0.1%
        pool_state.acceptable_price_difference = 50000;

        let source_amount_to_be_swapped: u128 = 1_000_000;
        let swap_source_amount: u128 = 10_000_000;
        let swap_destination_amount: u128 = 9_000_000;
        let oracle_price: u128 = 1_000_000_000;
        let spot_price: u128 = 1_030_000_000;

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        let max_swappable = swap_source_amount
            * pool_state.max_amount_swappable_at_oracle_price as u128
            / FEE_RATE_DENOMINATOR_VALUE as u128;
        assert_eq!(
            result, max_swappable,
            "Should be limited by max_amount_swappable_at_oracle_price"
        );

        // Test case 2: Very high acceptable_price_difference
        pool_state.max_amount_swappable_at_oracle_price = 100000;
        pool_state.acceptable_price_difference = 900000; // 90% difference allowed

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        assert!(
            result > 0,
            "Should allow swap with high acceptable_price_difference"
        );
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_boundary_conditions() {
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100000;
        pool_state.acceptable_price_difference = 50000;

        // Test case 1: Minimum possible non-zero values
        let source_amount_to_be_swapped: u128 = 1;
        let swap_source_amount: u128 = 1;
        let swap_destination_amount: u128 = 1;
        let oracle_price: u128 = 1_000_000_000;
        let spot_price: u128 = 1_000_000_000;

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        assert!(
            result <= source_amount_to_be_swapped,
            "Should handle minimum values correctly"
        );

        // Test case 2: Exact fee denominator values
        let source_amount_to_be_swapped: u128 = FEE_RATE_DENOMINATOR_VALUE as u128;
        let swap_source_amount: u128 = FEE_RATE_DENOMINATOR_VALUE as u128;
        let swap_destination_amount: u128 = FEE_RATE_DENOMINATOR_VALUE as u128;

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        assert!(
            result <= source_amount_to_be_swapped,
            "Should handle fee denominator values correctly"
        );

        // Test case 3: Values causing exact division
        let source_amount_to_be_swapped: u128 = 1_000_000;
        let swap_source_amount: u128 = 1_000_000;
        let swap_destination_amount: u128 = 1_000_000;
        let oracle_price: u128 = 1_000_000_000;
        let spot_price: u128 = 1_025_000_000; // 2.5% difference

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        assert!(result > 0, "Should handle exact division cases correctly");
    }

    #[test]
    fn test_get_amount_to_be_swapped_at_oracle_price_rounding() {
        let mut pool_state = PoolState::default();
        pool_state.max_amount_swappable_at_oracle_price = 100000;
        pool_state.acceptable_price_difference = 50000;

        // Test case 1: Values that would cause rounding in division
        let source_amount_to_be_swapped: u128 = 1000;
        let swap_source_amount: u128 = 1001; // Non-divisible number
        let swap_destination_amount: u128 = 999;
        let oracle_price: u128 = 1_000_000_123; // Non-round number
        let spot_price: u128 = 1_020_000_456; // Non-round number

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        assert!(
            result <= source_amount_to_be_swapped,
            "Should handle rounding correctly and not exceed source amount"
        );

        // Test case 2: Prime numbers to test division rounding
        let source_amount_to_be_swapped: u128 = 997; // Prime number
        let swap_source_amount: u128 = 1009; // Prime number
        let swap_destination_amount: u128 = 991; // Prime number

        let result = OracleBasedSwapCalculator::get_amount_to_be_swapped_at_oracle_price(
            source_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
            oracle_price,
            &pool_state,
            spot_price,
        )
        .unwrap();

        assert!(
            result <= source_amount_to_be_swapped,
            "Should handle prime number divisions correctly"
        );
    }
}
