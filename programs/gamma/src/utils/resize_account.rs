use anchor_lang::{
    prelude::*,
    system_program::{transfer, CreateAccount, Transfer},
};

use crate::error::GammaError;

pub fn dynamic_realloc_account<'a, T>(
    account: &mut Account<'a, T>,
    payer: &mut AccountInfo<'a>,
    system_program: &Program<'a, System>,
) -> Result<()>
where
    T: AccountDeserialize + AccountSerialize + Owner + Clone + AnchorSerialize,
{
    let space = account.try_to_vec()?.len() + 8;

    let lamports_required = Rent::get()?.minimum_balance(space);
    let lamports_present = account.to_account_info().lamports();

    if lamports_required > lamports_present {
        let lamports_to_transfer = lamports_required
            .checked_sub(lamports_present)
            .ok_or(GammaError::MathError)?;
        let transfer_accounts = Transfer {
            from: payer.to_account_info(),
            to: account.to_account_info(),
        };
        let transfer_ctx = CpiContext::new(system_program.to_account_info(), transfer_accounts);
        transfer(transfer_ctx, lamports_to_transfer)?;
    }

    account.to_account_info().realloc(space, true)?;

    Ok(())
}

pub fn create_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, CreateAccount<'info>>,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
) -> Result<()> {
    let ix = anchor_lang::solana_program::system_instruction::create_account(
        ctx.accounts.from.key,
        ctx.accounts.to.key,
        lamports,
        space,
        owner,
    );
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.from, ctx.accounts.to])
        .map_err(Into::into)
}
