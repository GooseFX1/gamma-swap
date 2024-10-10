use crate::{
    error::GammaError,
    states::{AmmConfig, AMM_CONFIG_SEED},
};
use anchor_lang::prelude::*;
use referral::cpi::accounts::InitializeProject;
use referral::cpi::initialize_project;
use referral::program::Referral;
use referral::InitializeProjectParams;

#[derive(Accounts)]
#[instruction(index: u16)]
pub struct CreateReferralProject<'info> {
    /// Address to be set as protocol owner.
    #[account(address = crate::admin::id() @ GammaError::InvalidOwner)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    /// The config acts as the base for its referral project
    #[account(mut)]
    pub amm_config: Account<'info, AmmConfig>,

    /// CHECK: The project account to be created
    pub project: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub referral_program: Program<'info, Referral>,
}

pub fn create_referral_project(
    ctx: Context<CreateReferralProject>,
    name: String,
    default_share_bps: u16,
) -> Result<()> {
    let config = &mut ctx.accounts.amm_config;
    config.referral_project = ctx.accounts.project.key();

    let seeds = &[
        AMM_CONFIG_SEED.as_bytes(),
        &config.index.to_be_bytes(),
        &[config.bump],
    ];
    let signer_seeds = &[&seeds[..]];

    let ctx = CpiContext::new_with_signer(
        ctx.accounts.referral_program.to_account_info(),
        InitializeProject {
            payer: ctx.accounts.payer.to_account_info(),
            base: ctx.accounts.amm_config.to_account_info(),
            admin: ctx.accounts.owner.to_account_info(),
            project: ctx.accounts.project.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        signer_seeds,
    );
    initialize_project(
        ctx,
        InitializeProjectParams {
            name,
            default_share_bps,
        },
    )?;

    Ok(())
}
