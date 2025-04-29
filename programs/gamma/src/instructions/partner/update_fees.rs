use crate::states::{PoolPartnerInfos, PoolState, PARTNER_INFOS_SEED};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdatePartnerFees<'info> {
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        mut,
        seeds = [PARTNER_INFOS_SEED.as_bytes(), pool_state.key().as_ref()],
        bump,
    )]
    pub pool_partners: AccountLoader<'info, PoolPartnerInfos>,
}

pub fn update_partner_fees(ctx: Context<UpdatePartnerFees>) -> Result<()> {
    let (fees_token_0, fees_token_1) = {
        let pool = ctx.accounts.pool_state.load()?;
        (
            pool.partner_protocol_fees_token_0,
            pool.partner_protocol_fees_token_1,
        )
    };

    let mut pool_partners = ctx.accounts.pool_partners.load_mut()?;
    pool_partners.update_fee_amounts(fees_token_0, fees_token_1)?;

    Ok(())
}
