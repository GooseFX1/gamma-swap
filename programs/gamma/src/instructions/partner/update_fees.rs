use crate::states::{PoolPartnerInfos, PoolState, PARTNER_INFOS_SEED};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdatePartnerFees<'info> {
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        mut,
        seeds = [PARTNER_INFOS_SEED.as_bytes(), pool_state.key().as_ref()],
        bump,
    )]
    pub pool_partners: AccountLoader<'info, PoolPartnerInfos>,
}

pub fn update_partner_fees(ctx: Context<UpdatePartnerFees>) -> Result<()> {
    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    let mut pool_partners = ctx.accounts.pool_partners.load_mut()?;
    pool_partners.update_fee_amounts(&mut pool_state)?;
    Ok(())
}
