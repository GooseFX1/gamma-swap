use anchor_lang::prelude::*;

use crate::states::{
    GlobalUserLpRecentChange, PartnerType, PoolState, UserPoolLiquidity, USER_POOL_LIQUIDITY_SEED,
};

#[derive(Accounts)]
pub struct InitUserPoolLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        init,
        seeds = [
            USER_POOL_LIQUIDITY_SEED.as_bytes(),
            pool_state.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
        payer = user,
        space = UserPoolLiquidity::LEN,
    )]
    pub user_pool_liquidity: Box<Account<'info, UserPoolLiquidity>>,

    #[account(
        init,
        space = GlobalUserLpRecentChange::MIN_SIZE,
        payer = user,
        seeds = [
            crate::GLOBAL_USER_LP_RECENT_CHANGE_SEED.as_bytes(),
            pool_state.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
    )]
    pub global_user_lp_recent_change: Box<Account<'info, GlobalUserLpRecentChange>>,

    /// To create a new program account
    pub system_program: Program<'info, System>,
}

pub fn init_user_pool_liquidity(
    ctx: Context<InitUserPoolLiquidity>,
    partner: Option<String>,
) -> Result<()> {
    let user_pool_liquidity = &mut ctx.accounts.user_pool_liquidity;

    let partner = match partner {
        Some(partner_value) => match partner_value.as_str() {
            "AssetDash" => Some(PartnerType::AssetDash),
            _ => None,
        },
        None => None,
    };

    user_pool_liquidity.initialize(
        ctx.accounts.user.key(),
        ctx.accounts.pool_state.key(),
        partner,
    );
    Ok(())
}
