use anchor_lang::prelude::*;

use super::PartnerType;

pub const USER_POOL_LIQUIDITY_SEED: &str = "user-pool-liquidity";

#[account]
#[derive(Default, Debug)]
pub struct UserPoolLiquidity {
    pub user: Pubkey,
    pub pool_state: Pubkey,
    pub token_0_deposited: u128,
    pub token_1_deposited: u128,
    pub token_0_withdrawn: u128,
    pub token_1_withdrawn: u128,
    pub lp_tokens_owned: u128,
    pub partner: Option<PartnerType>,
    pub padding: [u8; 23],
}

impl UserPoolLiquidity {
    pub const LEN: usize = 8 + 32 * 2 + 16 * 5 + 32;

    pub fn initialize(&mut self, user: Pubkey, pool_state: Pubkey, partner: Option<PartnerType>) {
        self.user = user;
        self.pool_state = pool_state;
        self.token_0_deposited = 0;
        self.token_1_deposited = 0;
        self.token_0_withdrawn = 0;
        self.token_1_withdrawn = 0;
        self.lp_tokens_owned = 0;
        self.partner = partner;
        self.padding = [0u8; 23];
    }
}
