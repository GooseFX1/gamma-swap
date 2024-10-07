use anchor_lang::prelude::*;

use crate::{error::GammaError, fees::FEE_RATE_DENOMINATOR_VALUE, states::AmmConfig};

#[derive(Accounts)]
pub struct UpdateAmmConfig<'info> {
    /// The amm config owner or admin
    #[account( address = crate::admin::id() @GammaError::InvalidOwner)]
    pub owner: Signer<'info>,

    /// The amm config account to update
    #[account(mut)]
    pub amm_config: Account<'info, AmmConfig>,
}

pub fn update_amm_config(ctx: Context<UpdateAmmConfig>, param: u8, value: u64) -> Result<()> {
    let amm_config = &mut ctx.accounts.amm_config;
    match param {
        0 => update_trade_fee_rate(amm_config, value),
        1 => update_protocol_fee_rate(amm_config, value),
        2 => update_fund_fee_rate(amm_config, value),
        3 => {
            let new_protocol_owner = match ctx.remaining_accounts.iter().next() {
                Some(account) => account.key(),
                None => return err!(GammaError::InvalidInput),
            };
            set_new_protocol_owner(amm_config, new_protocol_owner)?;
        }
        4 => {
            let new_fund_owner = match ctx.remaining_accounts.iter().next() {
                Some(account) => account.key(),
                None => return err!(GammaError::InvalidInput),
            };
            set_new_fund_owner(amm_config, new_fund_owner)?;
        }
        5 => amm_config.create_pool_fee = value,
        6 => amm_config.disable_create_pool = if value == 0 { false } else { true },
        _ => return err!(GammaError::InvalidInput),
    }

    Ok(())
}

fn update_trade_fee_rate(amm_config: &mut Account<AmmConfig>, trade_fee_rate: u64) {
    assert!(trade_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
    amm_config.trade_fee_rate = trade_fee_rate;
}

fn update_protocol_fee_rate(amm_config: &mut Account<AmmConfig>, protocol_fee_rate: u64) {
    assert!(protocol_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
    assert!(protocol_fee_rate + amm_config.fund_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
    amm_config.protocol_fee_rate = protocol_fee_rate;
}

fn update_fund_fee_rate(amm_config: &mut Account<AmmConfig>, fund_fee_rate: u64) {
    assert!(fund_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
    assert!(fund_fee_rate + amm_config.protocol_fee_rate <= FEE_RATE_DENOMINATOR_VALUE);
    amm_config.fund_fee_rate = fund_fee_rate;
}

fn set_new_protocol_owner(
    amm_config: &mut Account<AmmConfig>,
    new_protocol_owner: Pubkey,
) -> Result<()> {
    require_keys_neq!(amm_config.protocol_owner, new_protocol_owner);
    require_keys_neq!(new_protocol_owner, Pubkey::default());
    #[cfg(feature = "enable-log")]
    msg!(
        "amm_config, old_protocol_owner:{}, new_owner:{}",
        amm_config.protocol_owner.to_string(),
        new_protocol_owner.key().to_string()
    );
    amm_config.protocol_owner = new_protocol_owner;
    Ok(())
}

fn set_new_fund_owner(amm_config: &mut Account<AmmConfig>, new_fund_owner: Pubkey) -> Result<()> {
    require_keys_neq!(amm_config.fund_owner, new_fund_owner);
    require_keys_neq!(new_fund_owner, Pubkey::default());
    #[cfg(feature = "enable-log")]
    msg!(
        "amm_config, old_fund_owner:{}, new_owner:{}",
        amm_config.fund_owner.to_string(),
        new_fund_owner.key().to_string()
    );
    amm_config.fund_owner = new_fund_owner;
    Ok(())
}
