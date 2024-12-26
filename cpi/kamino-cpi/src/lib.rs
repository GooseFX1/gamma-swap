// From the kamnio IDL we have removed the refreshReservesBatch instruction, as the lifetime was not being used in that and it was failing to generate the CPI crate.
// Here is the removed instruction:
//  {
// "name": "refreshReservesBatch",
// "accounts": [],
// "args": [
//   {
//     "name": "skipPriceUpdates",
//     "type": "bool"
//   }
// ]
// }
// This should not cause any issues for making the cpi calls for other instructions.

use collateral_exchange_rate::CollateralExchangeRate;

declare_id!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

anchor_gen::generate_cpi_crate!("src/kamino.json");
mod collateral_exchange_rate;
mod fraction;
use fraction::Fraction;

pub struct Kamino;

impl Id for Kamino {
    fn id() -> Pubkey {
        ID
    }
}

impl ReserveLiquidity {
    fn total_supply(&self) -> Result<Fraction> {
        Ok(
            Fraction::from(self.available_amount) + Fraction::from_bits(self.borrowed_amount_sf)
                - Fraction::from_bits(self.accumulated_protocol_fees_sf)
                - Fraction::from_bits(self.accumulated_referrer_fees_sf)
                - Fraction::from_bits(self.pending_referrer_fees_sf),
        )
    }
}

impl ReserveCollateral {
    fn exchange_rate(&self, total_liquidity: Fraction) -> Result<CollateralExchangeRate> {
        let rate = if self.mint_total_supply == 0 || total_liquidity == Fraction::ZERO {
            Fraction::ONE
        } else {
            Fraction::from(self.mint_total_supply) / total_liquidity
        };

        Ok(CollateralExchangeRate(rate))
    }
}

impl Reserve {
    pub fn redeem_collateral_expected(&self, collateral_amount: u64) -> Result<u64> {
        let collateral_exchange_rate = self.collateral_exchange_rate()?;

        let liquidity_amount = collateral_exchange_rate.collateral_to_liquidity(collateral_amount);

        Ok(liquidity_amount)
    }

    fn collateral_exchange_rate(&self) -> Result<CollateralExchangeRate> {
        let total_liquidity = self.liquidity.total_supply()?;
        self.collateral.exchange_rate(total_liquidity)
    }
}
