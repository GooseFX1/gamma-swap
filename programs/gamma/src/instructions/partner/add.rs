use anchor_lang::prelude::*;

use crate::error::GammaError;
use crate::states::{AmmConfig, Partner, PoolPartnerInfos, PoolState, PARTNER_INFOS_SEED};

#[derive(Accounts)]
pub struct AddPartner<'info> {
    #[account(constraint = [amm_config.secondary_admin, crate::admin::id()].contains(&authority.key()))]
    pub authority: Signer<'info>,

    #[account(address = pool_state.load()?.amm_config)]
    pub amm_config: Account<'info, AmmConfig>,

    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        mut,
        seeds = [PARTNER_INFOS_SEED.as_bytes(), pool_state.key().as_ref()],
        bump,
    )]
    pub pool_partners: AccountLoader<'info, PoolPartnerInfos>,

    #[account(has_one = pool_state)]
    pub partner: Account<'info, Partner>,
}

pub fn add_partner(ctx: Context<AddPartner>) -> Result<()> {
    let mut partners = ctx.accounts.pool_partners.load_mut()?;

    if partners.has(&ctx.accounts.partner.key()) {
        return err!(GammaError::PartnerAlreadyExistsForPool);
    }

    let mut pool_state = ctx.accounts.pool_state.load_mut()?;
    // Update global `last_observed` variables so that the new partner is only
    // eligible accumulated after now
    partners.update_fee_amounts(&mut pool_state)?;
    partners.add_new(ctx.accounts.partner.key())?;

    Ok(())
}
