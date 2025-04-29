use anchor_lang::prelude::*;

use crate::states::{Partner, PoolState, PARTNER_INFOS_SEED};

#[derive(Accounts)]
pub struct InitializePartner<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub authority: Signer<'info>,

    pub pool_state: AccountLoader<'info, PoolState>,

    /// CHECK: Valid pda account which exists for pool-state
    #[account(
        mut,
        seeds = [PARTNER_INFOS_SEED.as_bytes(), pool_state.key().as_ref()],
        bump,
    )]
    pub pool_partners: UncheckedAccount<'info>,

    #[account(
        init,
        payer = payer,
        space = Partner::LEN,
    )]
    pub partner: Account<'info, Partner>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_partner(
    ctx: Context<InitializePartner>,
    name: [u8; 32],
    token_0_token_account: Pubkey,
    token_1_token_account: Pubkey,
) -> Result<()> {
    ctx.accounts.partner.set_inner(Partner {
        name,
        authority: *ctx.accounts.authority.key,
        pool_state: ctx.accounts.pool_state.key(),
        token_0_token_account,
        token_1_token_account,
    });

    Ok(())
}
