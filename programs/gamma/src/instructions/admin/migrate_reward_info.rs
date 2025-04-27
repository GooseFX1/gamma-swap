use crate::states::PoolState;
use crate::states::{AmmConfig, RewardInfo};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct MigrateRewardInfo<'info> {
    #[account(
        mut,
        constraint = check_authority(authority.key(), &amm_config)
    )]
    pub authority: Signer<'info>,

    #[account()]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(mut)]
    pub reward_info: Account<'info, RewardInfo>,

    #[account(
        constraint = amm_config.key() == pool_state.load()?.amm_config
    )]
    pub amm_config: Account<'info, AmmConfig>,

    pub system_program: Program<'info, System>,
}

fn check_authority(authority: Pubkey, amm_config: &AmmConfig) -> bool {
    return authority == amm_config.secondary_admin || authority == crate::admin::id();
}

// Admins have to pass the amount disbursed in the transaction, as there is no way to know this on chain.
pub fn migrate_reward_info(ctx: Context<MigrateRewardInfo>, amount_disbursed: u64) -> Result<()> {
    ctx.accounts.reward_info.amount_disbursed = amount_disbursed;
    Ok(())
}
