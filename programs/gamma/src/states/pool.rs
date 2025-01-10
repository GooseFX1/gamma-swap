use crate::error::GammaError;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use std::ops::{BitAnd, BitOr, BitXor};

// Seed to derive account address and signature
pub const POOL_SEED: &str = "pool";
pub const POOL_LP_MINT_SEED: &str = "pool_lp_mint";
pub const POOL_VAULT_SEED: &str = "pool_vault";
// This is for deriving the token account where kamino collateral is deposited
pub const POOL_KAMINO_DEPOSITS_SEED: &str = "pool_kamino_deposits";

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

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, AnchorDeserialize, AnchorSerialize)]
#[repr(u64)]
pub enum PartnerType {
    #[default]
    AssetDash = 0,
}

impl PartnerType {
    pub fn new(value: u64) -> Self {
        match value {
            0 => PartnerType::AssetDash,
            _ => PartnerType::AssetDash,
        }
    }
}

#[zero_copy(unsafe)]
#[repr(packed)]
#[derive(Default, Debug)]
pub struct PartnerInfo {
    pub partner_id: u64,
    // This stores the LP tokens that are linked with the partner, i.e owned by customers of the partner.
    pub lp_token_linked_with_partner: u64,

    // This keeps track of tvl_share * fee_we_earned_with_swap_token0
    pub cumulative_fee_total_times_tvl_share_token_0: u64,

    // This keeps track of tvl_share * fee_we_earned_with_swap_token1
    pub cumulative_fee_total_times_tvl_share_token_1: u64,
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
    /// latest dynamic fee rate
    pub latest_dynamic_fee_rate: u64,

    // if zero then default of 10%(100000) is used
    pub max_trade_fee_rate: u64,

    // if zero then default of 300_000 is used
    pub volatility_factor: u64,

    // excluding the fund fees and protocol fees
    // The current balance of token0 and token1 in the vault
    pub token_0_vault_amount: u64,
    pub token_1_vault_amount: u64,

    // Max fractions after dividing by 1_000_000, that can be shared with the platforms(eg. Kamino) for extra yield generation.
    pub max_shared_token0: u64,
    pub max_shared_token1: u64,

    // This will store the partner information, like how much token0 and token1 they was invested from their platforms.
    pub partners: [PartnerInfo; 1],

    // Keeps track of the absolute amount we put in kamino, in terms of the token0 or token1.
    // This is important to make sure that when kamino collateral price decreases in rate cases we don't deposit more.
    pub token_0_amount_in_kamino: u64,
    pub token_1_amount_in_kamino: u64,
    // To keep track of the profit we made from kamino, in terms of the token0 or token1.
    pub withdrawn_kamino_profit_token_0: u64,
    pub withdrawn_kamino_profit_token_1: u64,
    /// padding
    pub padding: [u64; 8],
}

impl PoolState {
    pub const LEN: usize = 8 + 10 * 32 + 5 * 1 + 7 * 8 + 16 * 4 + 23 * 8;

    pub fn initialize(
        &mut self,
        token_0_vault_amount: u64,
        token_1_vault_amount: u64,
        auth_bump: u8,
        lp_supply: u64,
        open_time: u64,
        max_trade_fee_rate: u64,
        volatility_factor: u64,
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
        self.latest_dynamic_fee_rate = 0;
        self.max_trade_fee_rate = max_trade_fee_rate;
        self.volatility_factor = volatility_factor;
        self.token_0_vault_amount = token_0_vault_amount;
        self.token_1_vault_amount = token_1_vault_amount;
        self.max_shared_token0 = 0;
        self.max_shared_token1 = 0;
        self.token_0_amount_in_kamino = 0;
        self.token_1_amount_in_kamino = 0;

        self.partners = [PartnerInfo::default(); 1];

        self.padding = [0u64; 8];
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

    pub fn vault_amount_without_fee(&self) -> Result<(u64, u64)> {
        Ok((self.token_0_vault_amount, self.token_1_vault_amount))
    }

    pub fn token_price_x32(&self) -> Result<(u128, u128)> {
        let (token_0_amount, token_1_amount) = self.vault_amount_without_fee()?;
        Ok((
            token_1_amount as u128 * Q32 as u128 / token_0_amount as u128,
            token_0_amount as u128 * Q32 as u128 / token_1_amount as u128,
        ))
    }
}
