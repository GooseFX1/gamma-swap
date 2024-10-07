use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use std::ops::{BitAnd, BitOr, BitXor};
use crate::error::GammaError;

// Seed to derive account address and signature
pub const POOL_SEED: &str = "pool";
pub const POOL_LP_MINT_SEED: &str = "pool_lp_mint";
pub const POOL_VAULT_SEED: &str = "pool_vault";

pub const Q32: u128 = (u32::MAX as u128) + 1; // 2^32

pub enum PoolStatusBitIndex {
    Deposit,
    Withdraw,
    Swap,
}

#[derive(PartialEq, Eq)]
pub enum PoolStatusBitFlag {
    Enable,
    Disable,
}

#[account(zero_copy(unsafe))]
#[repr(packed)]
#[derive(Default, Debug)]
pub struct PoolState {
    /// To which AmmConfig the pool belongs to
    pub amm_config: Pubkey,
    /// Pool Creator
    pub pool_creator: Pubkey,
    /// Vault to store Token A of the pool
    pub token_0_vault: Pubkey,
    /// Vault to store Token B of the pool
    pub token_1_vault: Pubkey,

    /// Pool tokens are issued when Token A or Token B are deposited
    /// Pool tokens can be withdrawn back to the original Token A or Token B
    // pub lp_mint: Pubkey,
    pub _padding1: [u8; 32],
    /// Mint info of Token A
    pub token_0_mint: Pubkey,
    /// Mint info of Token B
    pub token_1_mint: Pubkey,

    /// token_0 program
    pub token_0_program: Pubkey,
    /// token_1_program
    pub token_1_program: Pubkey,

    /// Observation account to store the oracle data
    pub observation_key: Pubkey,

    pub auth_bump: u8,

    /// Bitwise represenation of the state of the pool
    /// Bit0: 1 - Disable Deposit(value will be 1), 0 - Deposit can be done(normal)
    /// Bit1: 1 - Disable Withdraw(value will be 2), 0 - Withdraw can be done(normal)
    /// Bit2: 1 - Disable Swap(value will be 4), 0 - Swap can be done(normal)
    pub status: u8,

    /// lp_mint decimals
    // pub lp_mint_decimals: u8,
    pub _padding2: u8,
    /// mint0 and mint1 decimals
    pub mint_0_decimals: u8,
    pub mint_1_decimals: u8,

    /// True circulating supply of lp_mint tokens without burns and lock-ups
    pub lp_supply: u64,

    /// The amount of token_0 and token_1 owed to Liquidity Provider
    pub protocol_fees_token_0: u64,
    pub protocol_fees_token_1: u64,

    pub fund_fees_token_0: u64,
    pub fund_fees_token_1: u64,

    /// The timestamp allowed for swap in the pool
    pub open_time: u64,
    /// recent epoch
    pub recent_epoch: u64,
    /// Trade fees of token_0 after every swap
    pub cumulative_trade_fees_token_0: u128,
    /// Trade fees of token_1 after every swap
    pub cumulative_trade_fees_token_1: u128,
    /// Cummulative volume of token_0
    pub cumulative_volume_token_0: u128,
    /// Cummulative volume of token_1
    pub cumulative_volume_token_1: u128,
    /// Filter period determine high frequency trading time window.
    pub filter_period: u32,
    /// Decay period determine when the volatile fee start decay / decrease.
    pub decay_period: u32,
    /// Reduction factor controls the volatile fee rate decrement rate.
    pub reduction_factor: u32,
    /// Used to scale the variable fee component depending on the dynamic of the market
    pub variable_fee_control: u32,

    pub volatility_v2_base_fee: u64,
    pub volatility_v2_max_fee: u64,
    pub volatility_v2_volatility_factor: u64,
    pub volatility_v2_imbalance_factor: u64,
    /// padding
    pub padding: [u64; 17],
}

impl PoolState {
    pub const LEN: usize = 8 + 10 * 32 + 5 * 1 + 7 * 8 + 16 * 4 + 23 * 8;

    pub fn initialize(
        &mut self,
        auth_bump: u8,
        lp_supply: u64,
        open_time: u64,
        pool_creator: Pubkey,
        amm_config: Pubkey,
        token_0_vault: Pubkey,
        token_1_vault: Pubkey,
        token_0_mint: &InterfaceAccount<Mint>,
        token_1_mint: &InterfaceAccount<Mint>,
        observation_key: Pubkey,
    ) -> Result<()> {
        self.amm_config = amm_config.key();
        self.pool_creator = pool_creator.key();
        self.token_0_vault = token_0_vault;
        self.token_1_vault = token_1_vault;
        self.token_0_mint = token_0_mint.key();
        self.token_1_mint = token_1_mint.key();
        self.token_0_program = *token_0_mint.to_account_info().owner;
        self.token_1_program = *token_1_mint.to_account_info().owner;
        self.observation_key = observation_key;
        self.auth_bump = auth_bump;
        self.mint_0_decimals = token_0_mint.decimals;
        self.mint_1_decimals = token_1_mint.decimals;
        self.lp_supply = lp_supply;
        self.protocol_fees_token_0 = 0;
        self.protocol_fees_token_1 = 0;
        self.fund_fees_token_0 = 0;
        self.fund_fees_token_1 = 0;
        self.open_time = open_time;
        let clock = match Clock::get() {
            Ok(clock) => clock,
            Err(_) => return err!(GammaError::ClockError),
        };
        self.recent_epoch = clock.epoch;
        self.cumulative_trade_fees_token_0 = 0;
        self.cumulative_trade_fees_token_1 = 0;
        self.cumulative_volume_token_0 = 0;
        self.cumulative_volume_token_1 = 0;
        self.filter_period = 0;
        self.decay_period = 0;
        self.reduction_factor = 0;
        self.variable_fee_control = 0;
        self.volatility_v2_base_fee = 0;
        self.volatility_v2_max_fee = 0;
        self.volatility_v2_volatility_factor = 0;
        self.volatility_v2_imbalance_factor = 0;
        self.padding = [0u64; 17];
        Ok(())
    }

    pub fn set_status(&mut self, status: u8) {
        self.status = status
    }

    pub fn set_status_by_bit(&mut self, bit: PoolStatusBitIndex, flag: PoolStatusBitFlag) {
        let s = u8::from(1) << (bit as u8);
        if flag == PoolStatusBitFlag::Disable {
            self.status = self.status.bitor(s);
        } else {
            let m = u8::from(255).bitxor(s);
            self.status = self.status.bitand(m);
        }
    }

    // Get status by bit, if it is 'normal'/enabled return true
    pub fn get_status_by_bit(&self, bit: PoolStatusBitIndex) -> bool {
        let status = u8::from(1) << (bit as u8);
        self.status.bitand(status) == 0
    }

    pub fn vault_amount_without_fee(&self, vault_0: u64, vault_1: u64) -> Result<(u64, u64)> {
        Ok((
            vault_0
                .checked_sub(self.protocol_fees_token_0 + self.fund_fees_token_0)
                .ok_or(GammaError::MathOverflow)?,
            vault_1
                .checked_sub(self.protocol_fees_token_1 + self.fund_fees_token_1)
                .ok_or(GammaError::MathOverflow)?,
        ))
    }

    pub fn token_price_x32(&self, vault_0: u64, vault_1: u64) -> Result<(u128, u128)> {
        let (token_0_amount, token_1_amount) = self.vault_amount_without_fee(vault_0, vault_1)?;
        Ok((
            token_1_amount as u128 * Q32 as u128 / token_0_amount as u128,
            token_0_amount as u128 * Q32 as u128 / token_1_amount as u128,
        ))
    }

    #[inline(always)]
    pub fn get_filter_period(&self) -> u32 {
        self.filter_period
    }

    #[inline(always)]
    pub fn get_decay_period(&self) -> u32 {
        self.decay_period
    }
}
