use crate::error::GammaError;
use anchor_lang::prelude::*;

pub const PARTNER_SIZE: usize = 5;
pub const PARTNER_INFOS_SEED: &str = "partner_infos";

pub const MAX_NAME_LEN: usize = 20;

#[account]
/// Account storing information for a protocol participating in the Gamma partner program
pub struct Partner {
    /// Protocol name
    pub name: String,
    /// The authority for this account
    pub authority: Pubkey,
    /// The pool which this partner account belongs to
    pub pool_state: Pubkey,
    /// The token-account that receives token0 tokens
    pub token_0_token_account: Pubkey,
    /// The token-account that receives token1 tokens
    pub token_1_token_account: Pubkey,
}

impl Partner {
    pub const LEN: usize = 8 + /* discriminator */
        4 + 20 + /* name(max 20 bytes) */
        32 + /* authority */
        32 + /* pool_state */
        32 + /* token_0_token_account */
        32; /* token_1_token_account */
}

#[account(zero_copy(unsafe))]
#[repr(packed)]
#[derive(Default, Debug)]
/// PDA storing all the information for valid pool partners
pub struct PoolPartnerInfos {
    /// The observed fee-amount token0 as at the last infos update
    pub last_observed_fee_amount_token_0: u64,

    /// The observed fee-amount token1 as at the last infos update
    pub last_observed_fee_amount_token_1: u64,

    /// Partner infos
    pub infos: [PartnerInfo; PARTNER_SIZE],
}

impl PoolPartnerInfos {
    pub const LEN: usize = 8 /* discriminator */ + 8 /* u64 */ + 8 /* u64 */ + PARTNER_SIZE * PartnerInfo::LEN /* [PartnerInfo; PARTNER_SIZE] */ ;

    /// Initializes the `PartnerInfo` array with default values
    pub fn initialize(&mut self) -> Result<()> {
        self.infos = [PartnerInfo::default(); PARTNER_SIZE];
        Ok(())
    }

    /// Adds a `PartnerInfo` with a specific key to the infos array
    pub fn add_new(&mut self, partner: Pubkey) -> Result<()> {
        if let Some(entry) = self
            .infos
            .iter_mut()
            .find(|e| e.partner == Pubkey::default())
        {
            *entry = PartnerInfo {
                partner,
                ..Default::default()
            }
        } else {
            return err!(GammaError::ExceededMaxPartnersForPool);
        }

        Ok(())
    }

    /// Checks if the `PartnerInfo` for a particular pubkey exists
    pub fn has(&self, partner: &Pubkey) -> bool {
        self.info(partner).is_some()
    }

    /// Returns a shared reference to the `PartnerInfo` for a particular pubkey
    pub fn info(&self, partner: &Pubkey) -> Option<&PartnerInfo> {
        self.infos.iter().find(|p| p.partner == *partner)
    }

    /// Returns an exclusive reference to the `PartnerInfo` for a particular pubkey
    pub fn info_mut(&mut self, partner: &Pubkey) -> Option<&mut PartnerInfo> {
        self.infos.iter_mut().find(|p| p.partner == *partner)
    }

    pub fn total_partner_linked_lp_tokens(&self) -> u64 {
        self.infos
            .iter()
            .filter_map(|i| {
                if i.partner != Pubkey::default() {
                    Some(i.lp_token_linked_with_partner)
                } else {
                    None
                }
            })
            .sum::<u64>()
    }

    pub fn update_fee_amounts(
        &mut self,
        partner_protocol_fees_token_0: u64,
        partner_protocol_fees_token_1: u64,
    ) -> Result<()> {
        let total_partner_linked_lp_tokens = self.total_partner_linked_lp_tokens();
        if total_partner_linked_lp_tokens == 0 {
            return Ok(());
        }

        let last_observed_fee_amount_token_0 = self.last_observed_fee_amount_token_0;
        let last_observed_fee_amount_token_1 = self.last_observed_fee_amount_token_1;

        let infos = self
            .infos
            .iter_mut()
            .filter(|i| i.partner != Pubkey::default());

        for info in infos {
            let lp_token_linked_with_partner = info.lp_token_linked_with_partner;

            msg!(
                "token_0: ({} - {}) * ({} / {}",
                partner_protocol_fees_token_0,
                last_observed_fee_amount_token_0,
                lp_token_linked_with_partner,
                total_partner_linked_lp_tokens
            );
            let earnings_token_0_numerator = (partner_protocol_fees_token_0 as u128)
                .checked_sub(last_observed_fee_amount_token_0 as u128)
                .ok_or(GammaError::MathError)?
                .checked_mul(lp_token_linked_with_partner as u128)
                .ok_or(GammaError::MathError)?;
            let earnings_token_0 = earnings_token_0_numerator
                .checked_div(total_partner_linked_lp_tokens as u128)
                .and_then(|r| u64::try_from(r).ok())
                .ok_or(GammaError::MathError)?;
            msg!("token_0 earnings={}", earnings_token_0);

            msg!(
                "token_1: ({} - {}) * ({} / {}",
                partner_protocol_fees_token_1,
                last_observed_fee_amount_token_1,
                lp_token_linked_with_partner,
                total_partner_linked_lp_tokens
            );
            let earnings_token_1_numerator = (partner_protocol_fees_token_1 as u128)
                .checked_sub(last_observed_fee_amount_token_1 as u128)
                .ok_or(GammaError::MathError)?
                .checked_mul(lp_token_linked_with_partner as u128)
                .ok_or(GammaError::MathError)?;
            let earnings_token_1 = earnings_token_1_numerator
                .checked_div(total_partner_linked_lp_tokens as u128)
                .and_then(|r| u64::try_from(r).ok())
                .ok_or(GammaError::MathError)?;
            msg!("token_1 earnings={}", earnings_token_1);

            info.total_earned_fee_amount_token_0 = info
                .total_earned_fee_amount_token_0
                .checked_add(earnings_token_0)
                .ok_or(GammaError::MathOverflow)?;
            info.total_earned_fee_amount_token_1 = info
                .total_earned_fee_amount_token_1
                .checked_add(earnings_token_1)
                .ok_or(GammaError::MathOverflow)?;
        }

        self.last_observed_fee_amount_token_0 = partner_protocol_fees_token_0;
        self.last_observed_fee_amount_token_1 = partner_protocol_fees_token_1;

        Ok(())
    }
}

#[zero_copy(unsafe)]
#[repr(packed)]
#[derive(Default, Debug)]
pub struct PartnerInfo {
    /// The address of the partner account.
    pub partner: Pubkey,

    /// This stores the LP tokens that are linked with the partner, i.e owned by customers of the partner.
    pub lp_token_linked_with_partner: u64,

    /// The total fee-amount token0 claimed by the partner
    pub total_claimed_fee_amount_token_0: u64,

    /// The total fee-amount token1 claimed by the partner
    pub total_claimed_fee_amount_token_1: u64,

    /// The total fee-amount token0 calculated for the partner
    pub total_earned_fee_amount_token_0: u64,

    /// The total fee-amount token1 calculated for the partner
    pub total_earned_fee_amount_token_1: u64,
}

impl PartnerInfo {
    const LEN: usize = 32 + 5 * 8;
}
