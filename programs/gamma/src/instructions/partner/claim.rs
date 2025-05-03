use crate::error::GammaError;
use crate::states::{Partner, PoolPartnerInfos, PoolState, PARTNER_INFOS_SEED};
use crate::utils::transfer_from_pool_vault_to_user;
use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_interface::Mint;
use anchor_spl::token_interface::Token2022;
use anchor_spl::token_interface::TokenAccount;

#[derive(Accounts)]
pub struct ClaimPartnerFees<'info> {
    #[account(
        has_one = token_0_token_account,
        has_one = token_1_token_account,
    )]
    pub partner: Account<'info, Partner>,

    /// CHECK: pool vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub authority: UncheckedAccount<'info>,

    pub pool_state: AccountLoader<'info, PoolState>,

    /// The address that holds pool tokens for token_0
    #[account(
        mut,
        constraint = token_0_vault.key() == pool_state.load()?.token_0_vault
    )]
    pub token_0_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The address that holds pool tokens for token_1
    #[account(
        mut,
        constraint = token_1_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub token_1_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The mint of token_0 vault
    #[account(
        address = token_0_vault.mint
    )]
    pub vault_0_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The mint of token_1 vault
    #[account(
        address = token_1_vault.mint
    )]
    pub vault_1_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [PARTNER_INFOS_SEED.as_bytes(), pool_state.key().as_ref()],
        bump,
    )]
    pub pool_partners: AccountLoader<'info, PoolPartnerInfos>,

    #[account(mut)]
    pub token_0_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub token_1_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The SPL program to perform token transfers
    pub token_program: Program<'info, Token>,

    /// The SPL program 2022 to perform token transfers
    pub token_program_2022: Program<'info, Token2022>,
}

pub fn claim_partner_fees(ctx: Context<ClaimPartnerFees>) -> Result<()> {
    let mut pool_partners = ctx.accounts.pool_partners.load_mut()?;

    let auth_bump = {
        let pool_state = ctx.accounts.pool_state.load()?;
        pool_state.auth_bump
    };

    let Some(partner) = pool_partners.info_mut(&ctx.accounts.partner.key()) else {
        return err!(GammaError::InvalidPartner);
    };

    let amount_0 = partner
        .total_earned_fee_amount_token_0
        .checked_sub(partner.total_claimed_fee_amount_token_0)
        .ok_or(GammaError::MathOverflow)?;
    let amount_1 = partner
        .total_earned_fee_amount_token_1
        .checked_sub(partner.total_claimed_fee_amount_token_1)
        .ok_or(GammaError::MathOverflow)?;

    partner.total_claimed_fee_amount_token_0 = partner.total_earned_fee_amount_token_0;
    partner.total_claimed_fee_amount_token_1 = partner.total_earned_fee_amount_token_1;

    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_0_vault.to_account_info(),
        ctx.accounts.token_0_token_account.to_account_info(),
        ctx.accounts.vault_0_mint.to_account_info(),
        if ctx.accounts.vault_0_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        amount_0,
        ctx.accounts.vault_0_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
    )?;

    transfer_from_pool_vault_to_user(
        ctx.accounts.authority.to_account_info(),
        ctx.accounts.token_1_vault.to_account_info(),
        ctx.accounts.token_1_token_account.to_account_info(),
        ctx.accounts.vault_1_mint.to_account_info(),
        if ctx.accounts.vault_1_mint.to_account_info().owner == ctx.accounts.token_program.key {
            ctx.accounts.token_program.to_account_info()
        } else {
            ctx.accounts.token_program_2022.to_account_info()
        },
        amount_1,
        ctx.accounts.vault_1_mint.decimals,
        &[&[crate::AUTH_SEED.as_bytes(), &[auth_bump]]],
    )?;

    Ok(())
}
