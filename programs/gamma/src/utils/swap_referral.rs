use anchor_lang::prelude::*;
use referral::ReferralAccount;
use referral::REFERRAL_ATA_SEED;
use spl_token::state::{Account as SplTokenAccount, GenericTokenAccount};

use crate::error::GammaError;

pub const REFERRAL_SHARE_BPS: u64 = 10_000;
pub struct ReferralDetails<'c, 'info> {
    pub share_bps: u16,
    pub referral_token_account: &'c AccountInfo<'info>,
}

pub fn extract_referral_info<'c, 'info>(
    input_token_mint: Pubkey,
    project_key: Pubkey,
    referral_account: &'c Option<AccountInfo<'info>>,
    referral_token_account: &'c Option<AccountInfo<'info>>,
) -> Result<Option<ReferralDetails<'c, 'info>>> {
    // We take exactly two accounts:
    // 1. The referral account
    // 2. The referral token-account
    if referral_account.is_none() || referral_token_account.is_none() {
        return Ok(None);
    }
    let referral_account = referral_account.as_ref().unwrap();
    let referral_token_account = referral_token_account.as_ref().unwrap();

    // check: Referral account belongs to referral program and is for project
    require_keys_eq!(*referral_account.owner, referral::ID);
    let referral = ReferralAccount::try_deserialize(&mut &referral_account.data.borrow()[..])?;
    require_keys_eq!(project_key, referral.project);

    // check: Referral token account has the expected seeds
    let expect_token_account_key = Pubkey::find_program_address(
        &[
            REFERRAL_ATA_SEED,
            referral_account.key().as_ref(),
            input_token_mint.key().as_ref(),
        ],
        &referral::ID,
    )
    .0;
    require_keys_eq!(referral_token_account.key(), expect_token_account_key);

    // Referral token-account might not exist for this mint. Don't return an error in this case
    if **referral_token_account.try_borrow_lamports()? == 0 {
        return Ok(None);
    }

    // check: Referral token account is owned by the project
    let token_account_data = referral_token_account.data.borrow();
    let token_account_owner =
        <SplTokenAccount as GenericTokenAccount>::unpack_account_owner(&token_account_data[..])
            .ok_or(anchor_lang::error::Error::from(
                ProgramError::InvalidAccountData,
            ))?;
    require_keys_eq!(project_key, *token_account_owner);

    Ok(Some(ReferralDetails {
        share_bps: referral.share_bps, // the referral program guarantees that this is < 10_000
        referral_token_account,
    }))
}

pub struct ReferralResult {
    pub referral_amount: u64,
    pub amount_after_referral: u64,
}

impl<'c, 'info> ReferralDetails<'c, 'info> {
    pub fn get_referral_amount(&self, amount: u64) -> Result<ReferralResult> {
        let referral_amount = amount
            .checked_mul(self.share_bps as u64)
            .ok_or(GammaError::MathOverflow)?
            .checked_div(REFERRAL_SHARE_BPS)
            .unwrap_or(0);

        let amount_after_referral = amount.checked_sub(referral_amount).unwrap_or(0);

        Ok(ReferralResult {
            referral_amount,
            amount_after_referral,
        })
    }
}
