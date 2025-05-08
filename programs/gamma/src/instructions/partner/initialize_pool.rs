use crate::states::{PoolPartnerInfos, PoolState, PARTNER_INFOS_SEED};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializePoolPartners<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        init,
        seeds = [PARTNER_INFOS_SEED.as_bytes(), pool_state.key().as_ref()],
        bump,
        payer = payer,
        space = PoolPartnerInfos::LEN
    )]
    pub pool_partners: AccountLoader<'info, PoolPartnerInfos>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_pool_partners(ctx: Context<InitializePoolPartners>) -> Result<()> {
    let mut pool_partners = ctx.accounts.pool_partners.load_init()?;
    pool_partners.initialize()?;

    Ok(())
}
