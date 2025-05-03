use anchor_lang::prelude::*;

use crate::error::GammaError;
use crate::states::{
    PoolPartnerInfos, PoolState, UserPoolLiquidity, PARTNER_INFOS_SEED, USER_POOL_LIQUIDITY_SEED,
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
        mut,
        seeds = [PARTNER_INFOS_SEED.as_bytes(), pool_state.key().as_ref()],
        bump,
    )]
    pub pool_partners: AccountLoader<'info, PoolPartnerInfos>,

    /// To create a new program account
    pub system_program: Program<'info, System>,
}

pub fn init_user_pool_liquidity(
    ctx: Context<InitUserPoolLiquidity>,
    partner: Option<Pubkey>,
) -> Result<()> {
    let user_pool_liquidity = &mut ctx.accounts.user_pool_liquidity;

    if let Some(new_partner) = partner {
        // If partner is specified, check that partners account exists for this pool and contains the specified partner
        let partner_info = ctx.accounts.pool_partners.load()?;

        if !partner_info.has(&new_partner) {
            return err!(GammaError::InvalidPartner);
        }
    }

    let current_time = Clock::get()?.unix_timestamp as u64;

    user_pool_liquidity.initialize(
        ctx.accounts.user.key(),
        ctx.accounts.pool_state.key(),
        partner,
        current_time,
    );
    Ok(())
}
