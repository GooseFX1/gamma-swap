use crate::states::Partner;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct UpdatePartner<'info> {
    pub authority: Signer<'info>,

    #[account(mut, has_one = authority)]
    pub partner: Account<'info, Partner>,
}

pub fn update_partner(
    ctx: Context<UpdatePartner>,
    token_account_0: Option<Pubkey>,
    token_account_1: Option<Pubkey>,
) -> Result<()> {
    let partner = &mut ctx.accounts.partner;

    if let Some(token_account_0) = token_account_0 {
        partner.token_0_token_account = token_account_0;
    }
    if let Some(token_account_1) = token_account_1 {
        partner.token_1_token_account = token_account_1;
    }

    Ok(())
}
