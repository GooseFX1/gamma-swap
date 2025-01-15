use anchor_lang::prelude::*;

/// Emitted when deposit or withdraw
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
#[derive(Clone, Debug)]
pub struct LpChangeEvent {
    #[index]
    pub pool_id: Pubkey,
    pub lp_amount_before: u64,
    // vault_0 amount - trade_fees
    pub token_0_vault_before: u64,
    // vault_1 amount - trade_fees
    pub token_1_vault_before: u64,
    // calculate result without transfer fees
    pub token_0_amount: u64,
    // calculate result without transfer fees
    pub token_1_amount: u64,
    // transfer fee on token_0 using token extensions
    pub token_0_transfer_fee: u64,
    // transfer fee on token_1 using token extensions
    pub token_1_transfer_fee: u64,
    // 0: deposit, 1: withdraw
    pub change_type: u8,
}

// Emitted when swap
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
#[derive(Clone, Debug)]
pub struct SwapEvent {
    #[index]
    pub pool_id: Pubkey,
    /// pool vault - trade_fees
    pub input_vault_before: u64,
    /// pool_vault - trade_fees
    pub output_vault_before: u64,
    /// calculate result without transfer fees
    pub input_amount: u64,
    /// calculate result without transfer fees
    pub output_amount: u64,
    /// input mint for the swap
    pub input_mint: Pubkey,
    /// output mint for the swap
    pub output_mint: Pubkey,
    /// transfer fees on input token using token extensions
    pub input_transfer_fee: u64,
    /// transfer fees on output token using token extensions
    pub output_transfer_fee: u64,
    pub base_input: bool,
    /// dynamic_fees after this swap
    pub dynamic_fee: u128,
}

/// Emitted when migration
#[event]
#[cfg_attr(feature = "client", derive(Debug))]
#[derive(Clone, Debug)]
pub struct MigrationEvent {
    pub from_pool: Pubkey,
    pub to_pool: Pubkey,
    pub token_0_amount_withdrawn: u64,
    pub token_1_amount_withdrawn: u64,
    pub lp_tokens_migrated: u128,
}
