use super::{ceil_div, floor_div, FEE_RATE_DENOMINATOR_VALUE};

pub struct StaticFee {}

impl StaticFee {
    /// Calculate the trading fee in trading tokens
    pub fn trading_fee(amount: u128, trade_fee_rate: u64) -> Option<u128> {
        ceil_div(
            amount,
            u128::from(trade_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
    }

    /// Calculate the owner protocol fee in trading tokens
    pub fn protocol_fee(amount: u128, protocol_fee_rate: u64) -> Option<u128> {
        floor_div(
            amount,
            u128::from(protocol_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
    }

    /// Calculate the fund fee in trading tokens
    pub fn fund_fee(amount: u128, fund_fee_rate: u64) -> Option<u128> {
        floor_div(
            amount,
            u128::from(fund_fee_rate),
            u128::from(FEE_RATE_DENOMINATOR_VALUE),
        )
    }

    pub fn calculate_pre_trade_fee_amount(
        post_fee_amount: u128,
        trade_fee_rate: u64,
    ) -> Option<u128> {
        if trade_fee_rate == 0 {
            Some(post_fee_amount)
        } else {
            // x = pre_fee_amount (has to be calculated)
            // y = post_fee_amount
            // r = trade_fee_rate
            // D = FEE_RATE_DENOMINATOR_VALUE
            // y = x * (1 - r/ D)
            // y = x * ((D -r) / D)
            // x = y * D / (D - r)

            let numerator = post_fee_amount.checked_mul(u128::from(FEE_RATE_DENOMINATOR_VALUE))?;
            let denominator =
                u128::from(FEE_RATE_DENOMINATOR_VALUE).checked_sub(u128::from(trade_fee_rate))?;

            numerator
                .checked_add(denominator)?
                .checked_sub(1)?
                .checked_div(denominator)
        }
    }
}
