use crate::{
    fees::FEE_RATE_DENOMINATOR_VALUE,
    states::{PoolState, POOL_KAMINO_DEPOSITS_SEED},
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::instructions::ID as INSTRUCTION_SYSVAR_ID;
use anchor_spl::{
    token::Token,
    token_2022::Token2022,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use borsh::BorshDeserialize;
use kamino_cpi::Kamino;

#[derive(Accounts)]
pub struct Rebalance<'info> {
    // The signer for this instruction can be anyone, it does not have to a an admin.
    // The amount of withdraw or deposit is determined by calculations and config updated by admin
    // which makes this instruction very safe.
    // By allowing anyone to sign this instruction, we can in future allow anyone to rebalance the pool
    // then it can also happen very easily from the frontend or this instruction can be added to withdraw/deposit
    // transactions.
    #[account(mut)]
    pub signer: Signer<'info>,

    /// CHECK: pool vault authority
    #[account(
        seeds = [
            crate::AUTH_SEED.as_bytes(),
        ],
        bump,
    )]
    pub gamma_authority: UncheckedAccount<'info>,

    /// The program account of the pool in which the swap will be performed
    #[account(mut)]
    pub pool_state: AccountLoader<'info, PoolState>,

    /// The vault token account for token 0
    #[account(
        mut,
        constraint = token_vault.key() == pool_state.load()?.token_0_vault  || token_vault.key() == pool_state.load()?.token_1_vault
    )]
    pub token_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        address = token_vault.mint
    )]
    pub token_mint: Box<InterfaceAccount<'info, Mint>>,

    // Kamino deposit and withdraw related accounts.
    #[account(mut)]
    pub kamino_reserve: AccountInfo<'info>,

    #[account(mut)]
    pub kamino_lending_market: AccountInfo<'info>,

    #[account()]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    // This is where the collateral is deposited to.
    #[account(
        init_if_needed,
        seeds = [
            POOL_KAMINO_DEPOSITS_SEED.as_bytes(),
            pool_state.key().as_ref(),
            token_mint.key().as_ref(),
        ],
        bump,
        payer = signer,
        token::mint = reserve_collateral_mint,
        token::authority = gamma_authority,
    )]
    pub gamma_pool_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = INSTRUCTION_SYSVAR_ID )]
    pub instruction_sysvar_account: AccountInfo<'info>,

    #[account(
        constraint = liquidity_token_program.key() == *token_mint.to_account_info().owner
    )]
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    pub collateral_token_program: Program<'info, Token>,

    pub kamino_program: Program<'info, Kamino>,
    pub token_program: Program<'info, Token>,
    pub token_program_2022: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn rebalance_kamino<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Rebalance<'info>>,
) -> Result<()> {
    let deposit_withdraw_amounts = get_deposit_withdraw_amounts(
        ctx.accounts.pool_state.clone(),
        ctx.accounts.token_vault.clone(),
        ctx.accounts.kamino_reserve.clone(),
        ctx.accounts.gamma_pool_destination_collateral.clone(),
    )?;
    if deposit_withdraw_amounts.should_do_nothing {
        return Ok(());
    }

    let signer_seeds: &[&[&[u8]]] = &[&[
        crate::AUTH_SEED.as_bytes(),
        &[deposit_withdraw_amounts.pool_state_auth_bump],
    ]];

    if deposit_withdraw_amounts.should_deposit {
        deposit_in_kamino(
            ctx,
            deposit_withdraw_amounts.amount_to_deposit_withdraw,
            signer_seeds,
        )?;
    } else {
        withdraw_from_kamino(
            ctx,
            deposit_withdraw_amounts.amount_to_deposit_withdraw,
            signer_seeds,
        )?;
    }

    Ok(())
}

struct DepositWithdrawAmountResult {
    pool_state_auth_bump: u8,
    should_deposit: bool,
    should_do_nothing: bool,
    // it is amount to deposit or withdraw
    amount_to_deposit_withdraw: u64,
}

fn get_deposit_withdraw_amounts<'c, 'info>(
    pool_state: AccountLoader<'info, PoolState>,
    token_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    kamino_reserve: AccountInfo<'info>,
    gamma_pool_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
) -> Result<DepositWithdrawAmountResult> {
    let pool_state = pool_state.load()?;
    let is_token_0 = token_vault.key() == pool_state.token_0_vault;

    let reserve: kamino_cpi::state::Reserve = load_account(&kamino_reserve)?;

    let collateral_amount = gamma_pool_destination_collateral.amount;

    let amount_deposited = reserve.redeem_collateral_expected(collateral_amount)?;

    let max_deposit_allowed_rate = if is_token_0 {
        pool_state.max_shared_token0
    } else {
        pool_state.max_shared_token1
    };
    // Get the original amount in the pool vault
    let amount_in_pool_vault = if is_token_0 {
        pool_state.token_0_vault_amount
    } else {
        pool_state.token_1_vault_amount
    };
    let max_deposit_allowed =
        amount_in_pool_vault * FEE_RATE_DENOMINATOR_VALUE / max_deposit_allowed_rate;

    msg!("max_deposit_allowed: {}", max_deposit_allowed);
    msg!("amount_deposited: {}", amount_deposited);
    msg!("amount_in_pool_vault: {}", amount_in_pool_vault);
    msg!("max_deposit_allowed_rate: {}", max_deposit_allowed_rate);
    msg!("collateral_amount: {}", collateral_amount);
    msg!("is_token_0: {}", is_token_0);

    Ok(DepositWithdrawAmountResult {
        pool_state_auth_bump: pool_state.auth_bump,
        should_do_nothing: max_deposit_allowed == amount_deposited,
        should_deposit: max_deposit_allowed < amount_deposited,
        amount_to_deposit_withdraw: if max_deposit_allowed < amount_deposited {
            amount_deposited.checked_sub(max_deposit_allowed).unwrap()
        } else {
            max_deposit_allowed.checked_sub(amount_deposited).unwrap()
        },
    })
}

pub fn load_account<T: BorshDeserialize>(account_info: &AccountInfo) -> Result<T> {
    let data = account_info.data.borrow();

    // Ensure data length matches the struct size
    if data.len() != std::mem::size_of::<T>() {
        panic!("Invalid account data");
        // return err!(ErrorCode::);
    }

    // Deserialize using Borsh
    T::try_from_slice(&data).map_err(|_| panic!("Invalid account data"))
}

pub fn deposit_in_kamino<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Rebalance<'info>>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let kamino_deposit_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.kamino_program.to_account_info(),
        kamino_cpi::cpi::accounts::DepositReserveLiquidity {
            owner: ctx.accounts.gamma_authority.to_account_info(),
            reserve: ctx.accounts.kamino_reserve.to_account_info(),
            lending_market: ctx.accounts.kamino_lending_market.to_account_info(),
            lending_market_authority: ctx.accounts.lending_market_authority.to_account_info(),
            reserve_liquidity_mint: ctx.accounts.token_mint.to_account_info(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.to_account_info(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.to_account_info(),
            user_source_liquidity: ctx.accounts.token_vault.to_account_info(),
            user_destination_collateral: ctx
                .accounts
                .gamma_pool_destination_collateral
                .to_account_info(),
            collateral_token_program: ctx.accounts.collateral_token_program.to_account_info(),
            liquidity_token_program: ctx.accounts.liquidity_token_program.to_account_info(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.to_account_info(),
        },
        signer_seeds,
    );
    kamino_cpi::cpi::deposit_reserve_liquidity(kamino_deposit_cpi_ctx, amount)?;
    Ok(())
}

pub fn withdraw_from_kamino<'c, 'info>(
    ctx: Context<'_, '_, 'c, 'info, Rebalance<'info>>,
    amount: u64,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let kamino_withdraw_cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.kamino_program.to_account_info(),
        kamino_cpi::cpi::accounts::RedeemReserveCollateral {
            owner: ctx.accounts.gamma_authority.to_account_info(),
            reserve: ctx.accounts.kamino_reserve.to_account_info(),
            lending_market: ctx.accounts.kamino_lending_market.to_account_info(),
            reserve_liquidity_mint: ctx.accounts.token_mint.to_account_info(),
            reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.to_account_info(),
            lending_market_authority: ctx.accounts.lending_market_authority.to_account_info(),
            reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.to_account_info(),
            user_source_collateral: ctx
                .accounts
                .gamma_pool_destination_collateral
                .to_account_info(),
            user_destination_liquidity: ctx.accounts.token_vault.to_account_info(),
            collateral_token_program: ctx.accounts.collateral_token_program.to_account_info(),
            liquidity_token_program: ctx.accounts.liquidity_token_program.to_account_info(),
            instruction_sysvar_account: ctx.accounts.instruction_sysvar_account.to_account_info(),
        },
        signer_seeds,
    );

    kamino_cpi::cpi::redeem_reserve_collateral(kamino_withdraw_cpi_ctx, amount)?;
    Ok(())
}
