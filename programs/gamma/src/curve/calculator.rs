//! Swap calculations

use crate::error::GammaError;
use crate::fees::{DynamicFee, FeeType};
use crate::states::ObservationState;
use crate::{curve::constant_product::ConstantProductCurve, fees::StaticFee};
use anchor_lang::prelude::*;
use std::fmt::Debug;

/// Helper function for mapping to GammaError::CalculationFailure
pub fn map_zero_to_none(x: u128) -> Option<u128> {
    if x == 0 {
        None
    } else {
        Some(x)
    }
}

/// The direction of a trade, since curves can be specialized to treat each
/// token differently (by adding offsets or weights)
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TradeDirection {
    /// Input token 0, output token 1
    ZeroForOne,
    /// Input token 1, output token 0
    OneForZero,
}

impl TradeDirection {
    /// Given a trade direction, gives the opposite direction of the trade, so
    /// A to B becomes B to A, and vice versa
    pub fn opposite(&self) -> TradeDirection {
        match self {
            TradeDirection::ZeroForOne => TradeDirection::OneForZero,
            TradeDirection::OneForZero => TradeDirection::ZeroForOne,
        }
    }
}

/// The direction to round.  Used for pool token to trading token conversions to
/// avoid losing value on any deposit or withdrawal.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoundDirection {
    /// Floor the value, ie. 1.9 => 1, 1.1 => 1, 1.5 => 1
    Floor,
    /// Ceiling the value, ie. 1.9 => 2, 1.1 => 2, 1.5 => 2
    Ceiling,
}

/// Encodes results of depositing both sides at once
#[derive(Debug, PartialEq)]
pub struct TradingTokenResult {
    /// Amount of token A
    pub token_0_amount: u128,
    /// Amount of token B
    pub token_1_amount: u128,
}

/// Encodes all results of swapping from a source token to a destination token
#[derive(Debug, PartialEq)]
pub struct SwapResult {
    /// New amount of source token
    pub new_swap_source_amount: u128,
    /// New amount of destination token
    pub new_swap_destination_amount: u128,
    /// Amount of source token swapped (includes fees)
    pub source_amount_swapped: u128,
    /// Amount of destination token swapped
    pub destination_amount_swapped: u128,
    /// Dynamic fee charged for trade
    pub dynamic_fee: u128,
    /// Amount of source tokens going to protocol
    pub protocol_fee: u128,
    /// Amount of source tokens going to protocol team
    pub fund_fee: u128,
}

/// Concrete struct to wrap around the trait object which performs calculation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurveCalculator {}

impl CurveCalculator {
    pub fn validate_supply(token_0_amount: u64, token_1_amount: u64) -> Result<()> {
        if token_0_amount == 0 {
            return Err(GammaError::EmptySupply.into());
        }
        if token_1_amount == 0 {
            return Err(GammaError::EmptySupply.into());
        }
        Ok(())
    }

    /// Subtract fees and calculate how much destination token will be received
    /// for a given amount of source token

    pub fn swap_base_input(
        source_amount_to_be_swapped: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        trade_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        block_timestamp: u64,
        observation_state: &ObservationState,
        trade_direction: TradeDirection,
        // TODO: add fee type here once that is configurable on pool level/ or we can use it from pool_state
    ) -> Result<SwapResult> {
        let vault0 = match trade_direction {
            TradeDirection::ZeroForOne => swap_source_amount,
            TradeDirection::OneForZero => swap_destination_amount,
        };
        let vault1 = match trade_direction {
            TradeDirection::ZeroForOne => swap_destination_amount,
            TradeDirection::OneForZero => swap_source_amount,
        };

        let dynamic_fee = DynamicFee::dynamic_fee(
            source_amount_to_be_swapped,
            block_timestamp,
            observation_state,
            vault0 as u64,
            vault1 as u64,
            FeeType::Volatility,
            trade_fee_rate,
        )?;

        let protocol_fee = StaticFee::protocol_fee(dynamic_fee, protocol_fee_rate)
            .ok_or(GammaError::InvalidFee)?;
        let fund_fee = StaticFee::fund_fee(dynamic_fee, fund_fee_rate)
            .ok_or(GammaError::InvalidFee)?;

        let source_amount_after_fees = source_amount_to_be_swapped
            .checked_sub(dynamic_fee)
            .ok_or(GammaError::MathOverflow)?;
        let destination_amount_swapped = ConstantProductCurve::swap_base_input_without_fees(
            source_amount_after_fees,
            swap_source_amount,
            swap_destination_amount,
        )?;

        Ok(SwapResult {
            new_swap_source_amount: swap_source_amount
                .checked_add(source_amount_to_be_swapped)
                .ok_or(GammaError::MathOverflow)?,
            new_swap_destination_amount: swap_destination_amount
                .checked_sub(destination_amount_swapped)
                .ok_or(GammaError::MathOverflow)?,
            source_amount_swapped: source_amount_to_be_swapped,
            destination_amount_swapped,
            dynamic_fee,
            protocol_fee,
            fund_fee,
        })
    }

    /// Subtract fees and calculate how much source token will be required
    pub fn swap_base_output(
        destination_amount_to_be_swapped: u128,
        swap_source_amount: u128,
        swap_destination_amount: u128,
        trade_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        block_timestamp: u64,
        observation_state: &ObservationState,
        trade_direction: TradeDirection,
    ) -> Result<SwapResult> {
        let source_amount_swapped = ConstantProductCurve::swap_base_output_without_fees(
            destination_amount_to_be_swapped,
            swap_source_amount,
            swap_destination_amount,
        )?;

        let vault0 = match trade_direction {
            TradeDirection::ZeroForOne => swap_source_amount,
            TradeDirection::OneForZero => swap_destination_amount,
        } as u64;
        let vault1 = match trade_direction {
            TradeDirection::ZeroForOne => swap_destination_amount,
            TradeDirection::OneForZero => swap_source_amount,
        } as u64;

        let source_amount = DynamicFee::calculate_pre_fee_amount(
            block_timestamp,
            source_amount_swapped,
            observation_state,
            vault0,
            vault1,
            FeeType::Volatility,
            trade_fee_rate,
        )?;

        let dynamic_fee = source_amount
            .checked_sub(source_amount_swapped)
            .ok_or(GammaError::MathOverflow)?;
        let protocol_fee = StaticFee::protocol_fee(dynamic_fee, protocol_fee_rate)
            .ok_or(GammaError::MathOverflow)?;
        let fund_fee = StaticFee::fund_fee(dynamic_fee, fund_fee_rate)
            .ok_or(GammaError::MathOverflow)?;

        Ok(SwapResult {
            new_swap_source_amount: swap_source_amount
                .checked_add(source_amount)
                .ok_or(GammaError::MathOverflow)?,
            new_swap_destination_amount: swap_destination_amount
                .checked_sub(destination_amount_to_be_swapped)
                .ok_or(GammaError::MathOverflow)?,
            source_amount_swapped: source_amount,
            destination_amount_swapped: destination_amount_to_be_swapped,
            protocol_fee,
            fund_fee,
            dynamic_fee,
        })
    }

    /// Get the amount of trading tokens for the given amount of pool tokens
    /// provided the total trading tokens and supply of pool tokens
    pub fn lp_tokens_to_trading_tokens(
        lp_token_amount_to_be_exchanged: u128,
        lp_token_supply: u128,
        swap_token_0_amount: u128,
        swap_token_1_amount: u128,
        round_direction: RoundDirection,
    ) -> Option<TradingTokenResult> {
        ConstantProductCurve::lp_tokens_to_trading_tokens(
            lp_token_amount_to_be_exchanged,
            lp_token_supply,
            swap_token_0_amount,
            swap_token_1_amount,
            round_direction,
        )
    }
}