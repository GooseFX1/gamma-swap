use crate::{
    fees::FEE_RATE_DENOMINATOR_VALUE,
    states::{PoolPartnerInfos, PoolState, PARTNER_INFOS_SEED},
};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitializePoolPartners<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
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

pub fn initialize_pool_partners(
    ctx: Context<InitializePoolPartners>,
    partner_share_rate: u64,
) -> Result<()> {
    let mut pool = ctx.accounts.pool_state.load_mut()?;
    require_gte!(FEE_RATE_DENOMINATOR_VALUE, partner_share_rate);

    pool.partner_share_rate = partner_share_rate;
    pool.partner_protocol_fees_token_0 = 0;
    pool.partner_protocol_fees_token_1 = 0;

    let mut pool_partners = ctx.accounts.pool_partners.load_init()?;
    pool_partners.initialize()?;

    Ok(())
}
