use anchor_lang::prelude::*;

use crate::states::{PoolState, UserPoolLiquidity, USER_POOL_LIQUIDITY_SEED};

#[derive(Accounts)]
pub struct InitUserPoolLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    pub pool_state: AccountLoader<'info, PoolState>,

    #[account(
        init,
        seeds = [
            USER_POOL_LIQUIDITY_SEED.as_bytes(),
            pool_state.key().as_ref(),
            user.key().as_ref(),
        ],
        bump,
        payer = user,
        space = UserPoolLiquidity::LEN,
    )]
    pub user_pool_liquidity: Box<Account<'info, UserPoolLiquidity>>,

    /// To create a new program account
    pub system_program: Program<'info, System>,
}

pub fn init_user_pool_liquidity(ctx: Context<InitUserPoolLiquidity>) -> Result<()> {
    let user_pool_liquidity = &mut ctx.accounts.user_pool_liquidity;
    user_pool_liquidity.initialize(ctx.accounts.user.key(), ctx.accounts.pool_state.key());
    Ok(())
}
