pub mod orca_whirlpool_to_gamma;
pub mod orca_whirlpool_to_gamma_v2;

pub use orca_whirlpool_to_gamma::*;
pub use orca_whirlpool_to_gamma_v2::*;

use anchor_lang::prelude::*;
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum AccountsType {
    TransferHookA,
    TransferHookB,
    TransferHookReward,
    TransferHookInput,
    TransferHookIntermediate,
    TransferHookOutput,
    SupplementalTickArrays,
    SupplementalTickArraysOne,
    SupplementalTickArraysTwo,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RemainingAccountsSlice {
    pub accounts_type: AccountsType,
    pub length: u8,
}

// #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
// pub struct RemainingAccountsInfo {
//     pub slices: Vec<RemainingAccountsSlice>,
// }
