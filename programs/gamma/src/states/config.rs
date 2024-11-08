use anchor_lang::prelude::*;

use crate::fees::FEE_RATE_DENOMINATOR_VALUE;

pub const AMM_CONFIG_SEED: &str = "amm_config";

#[account]
#[derive(Default, Debug)]
pub struct AmmConfig {
    // Bump to identify PDA
    pub bump: u8,
    // Status to control if new pool can be created
    pub disable_create_pool: bool,
    /// Config index
    pub index: u16,
    // This is used as base fees in dynamic fees calculation
    /// The trade fee, denominated in hundredths of bip (10^-6)
    pub trade_fee_rate: u64,
    /// The protocol fee
    pub protocol_fee_rate: u64,
    /// The fund fee, denominated in hundredths of bip (10^-6)
    pub fund_fee_rate: u64,
    /// Fee for creating a new pool
    pub create_pool_fee: u64,
    /// Address of the protocol fee owner
    pub protocol_owner: Pubkey,
    /// Address of the fund fee owner
    pub fund_owner: Pubkey,
    /// Address of the referral project
    pub referral_project: Pubkey,
    /// Max open time for a pool in seconds
    pub max_open_time: u64,
    /// padding
    pub padding: [u64; 11],
}

impl AmmConfig {
    pub const LEN: usize = 8 + 1 + 1 + 2 + 4 * 8 + 2 * 32 + 8 * 16;
}

// require all rates to be less than 1 (100%)
pub fn validate_config_rates(amm_config: &AmmConfig) -> Result<()> {
    require_gt!(FEE_RATE_DENOMINATOR_VALUE, amm_config.trade_fee_rate);
    require_gt!(FEE_RATE_DENOMINATOR_VALUE, amm_config.protocol_fee_rate);
    require_gt!(FEE_RATE_DENOMINATOR_VALUE, amm_config.fund_fee_rate);
    require_gt!(
        FEE_RATE_DENOMINATOR_VALUE,
        amm_config.fund_fee_rate + amm_config.protocol_fee_rate
    );

    Ok(())
}
