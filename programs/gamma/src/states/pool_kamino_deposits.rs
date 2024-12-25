use anchor_lang::prelude::*;

use crate::error::GammaError;

pub const POOL_KAMINO_DEPOSITS_SEED: &str = "pool-kamino-deposits";
/*
 Only call in business hours
 

*/
// THis account is to store how much of the amount of token_mint is deposited in Kamino for a pool
// The key is derived from `[pool_id, token_mint, POOL_KAMINO_DEPOSITS_SEED]`
#[account]
#[derive(Default, Debug)]
pub struct PoolTokenExternalDeposits {
    pub pool_id: Pubkey,
    pub token_mint: Pubkey,
    pub total_deposits_in_external_protocol: u64,
    
    // made private to prevent updates without the use of the `change_kamino_market` function
    kamino_market: Pubkey,
    pub padding: [u8; 23],
}

impl PoolTokenExternalDeposits {
    pub const LEN: usize = 8 + 32 * 2 + 16 * 5 + 32;

    pub fn initialize(&mut self, pool_id: Pubkey, token_mint: Pubkey, kamino_market: Pubkey) {
        self.pool_id = pool_id;
        self.token_mint = token_mint;
        self.kamino_market = kamino_market;
        self.amount_deposited = 0;
    }

    pub fn update_amount_deposited(&mut self, amount_deposited: u64) {
        self.amount_deposited = amount_deposited;
    }

    pub fn change_kamino_market(&mut self, kamino_market: Pubkey) -> Result<()> {
        if self.amount_deposited > 0 {
            msg!("Cannot change kamino market when there is already a deposit");
            return err!(GammaError::CannotChangeKaminoMarket);
        }
        self.kamino_market = kamino_market;
        Ok(())
    }

    pub fn get_kamino_market(&self) -> Pubkey {
        self.kamino_market
    }
}
