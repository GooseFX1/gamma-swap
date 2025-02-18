pub mod math;
pub mod resize_account;
pub mod swap_referral;
pub mod token;
pub use math::*;

pub use resize_account::*;
pub use swap_referral::*;
pub use token::*;

pub const MAX_REWARDS: usize = 3;
pub const SECONDS_IN_A_DAY: u64 = 86400;
