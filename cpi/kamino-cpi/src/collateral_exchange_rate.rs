use crate::fraction::{Fraction, FractionExtra};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollateralExchangeRate(pub Fraction);

impl CollateralExchangeRate {
    pub fn collateral_to_liquidity(&self, collateral_amount: u64) -> u64 {
        self.fraction_collateral_to_liquidity(collateral_amount.into())
            .to_floor()
    }

    pub fn fraction_collateral_to_liquidity(&self, collateral_amount: Fraction) -> Fraction {
        collateral_amount / self.0
    }

    pub fn fraction_liquidity_to_collateral(&self, liquidity_amount: Fraction) -> Fraction {
        self.0 * liquidity_amount
    }

    pub fn liquidity_to_collateral_fraction(&self, liquidity_amount: u64) -> Fraction {
        self.0 * u128::from(liquidity_amount)
    }

    pub fn liquidity_to_collateral(&self, liquidity_amount: u64) -> u64 {
        self.liquidity_to_collateral_fraction(liquidity_amount)
            .to_floor()
    }

    pub fn liquidity_to_collateral_ceil(&self, liquidity_amount: u64) -> u64 {
        self.liquidity_to_collateral_fraction(liquidity_amount)
            .to_ceil()
    }
}

impl From<CollateralExchangeRate> for Fraction {
    fn from(exchange_rate: CollateralExchangeRate) -> Self {
        exchange_rate.0
    }
}

impl From<Fraction> for CollateralExchangeRate {
    fn from(fraction: Fraction) -> Self {
        Self(fraction)
    }
}
