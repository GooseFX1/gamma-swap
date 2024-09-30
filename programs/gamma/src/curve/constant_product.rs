//! The Uniswap invariantConstantProductCurve::

use crate::{
    curve::calculator::{RoundDirection, TradingTokenResult},
    utils::CheckedCeilDiv,
};

/// ConstantProductCurve struct implementing CurveCalculator
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConstantProductCurve;

impl ConstantProductCurve {
    /// ConstantProduct swap ensures x * y = constant
    /// The constant product swap calculation, factored out of it's class for reuse.
    ///
    /// This is guaranteed to work for all the values such that
    /// 1 <= swap_source_amount * swap_destination_amount <= u128::MAX
    /// 1 <= source_amount <= u64::MAX
    pub fn swap_base_input_without_fees(
        source_amount_to_be_swapped: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
    ) -> u128 {
        // (x + delta_x) * (y - delta_y) = x * y
        // delta_y = (delta_x * y) / (x + delta_x)
        let numerator = source_amount_to_be_swapped
            .checked_mul(swap_destination_amount)
            .unwrap();
        let denominator = swap_source_amount
            .checked_add(source_amount_to_be_swapped)
            .unwrap();
        let destination_amount_swapped = numerator.checked_div(denominator).unwrap();
        destination_amount_swapped
    }

    pub fn swap_base_output_without_fees(
        destination_amount_to_be_swapped: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
    ) -> u128 {
        // (x + delta_x) * (y - delta_y) = x * y
        // delta_x = (x * delta_y) / (y - delta_y)
        let numerator = swap_source_amount
            .checked_mul(destination_amount_to_be_swapped)
            .unwrap();
        let denominator = swap_destination_amount
            .checked_sub(destination_amount_to_be_swapped)
            .unwrap();
        let (source_amount_swapped, _) = numerator.checked_ceil_div(denominator).unwrap();
        source_amount_swapped
    }

    /// Get the amount of trading tokens(token_0 and token_1) for a given amount of pool tokens(lp_tokens)
    /// provided the total trading tokens and supply of pool tokens
    ///
    /// The constant product implementation is a simple ratio calcluations for the amount of trading tokens
    /// corresponding to a certain number of pool tokens.
    pub fn lp_tokens_to_trading_tokens(
        lp_token_amount: u128,
        lp_token_supply: u128,
        swap_token_0_amount: u128,
        swap_token_1_amount: u128,
        round_direction: RoundDirection,
    ) -> Option<TradingTokenResult> {
        // token_0_amount = (lp_token_amount * swap_token_0_amount) / lp_token_supply
        // lp_token_amount - Amount of pool tokens to be exchanged
        // swap_token_0_amount - Total token_0 amount in the pool
        let mut token_0_amount = lp_token_amount
            .checked_mul(swap_token_0_amount)?
            .checked_div(lp_token_supply)?;
        let mut token_1_amount = lp_token_amount
            .checked_mul(swap_token_1_amount)?
            .checked_div(lp_token_supply)?;
        let (token_0_amount, token_1_amount) = match round_direction {
            RoundDirection::Floor => (token_0_amount, token_1_amount),
            RoundDirection::Ceiling => {
                let token_0_remainder = lp_token_amount
                    .checked_mul(swap_token_0_amount)?
                    .checked_rem(lp_token_supply)?;
                // Also check for 0 token A and B amount to avoid taking too much
                // for tiny amounts of pool tokens.  For example, if someone asks
                // for 1 pool token, which is worth 0.01 token A, we avoid the
                // ceiling of taking 1 token A and instead return 0, for it to be
                // rejected later in processing.
                if token_0_remainder > 0 && token_0_amount > 0 {
                    token_0_amount = token_0_amount.checked_add(1)?;
                }
                let token_1_remainder = lp_token_amount
                    .checked_mul(swap_token_1_amount)?
                    .checked_rem(lp_token_supply)?;
                if token_1_remainder > 0 && token_1_amount > 0 {
                    token_1_amount = token_1_amount.checked_add(1)?;
                }
                (token_0_amount, token_1_amount)
            }
        };
        Some(TradingTokenResult {
            token_0_amount,
            token_1_amount,
        })
    }
}
