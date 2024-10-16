use anchor_lang::prelude::*;
use dlmm::{
    instructions::remove_liquidity::BinLiquidityReduction,
    instruction::RemoveLiquidity,
};
use solana_program::{instruction::Instruction, program::invoke};
use anchor_lang::solana_program::pubkey::Pubkey;
use bincode;

#[derive(Accounts)]
pub struct DlmmToGamma<'info> {
    #[account(mut)]
    /// CHECK: The position account
    pub position: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: The pool account
    pub lb_pair: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Bin array extension account of the pool
    pub bin_array_bitmap_extension: Option<UncheckedAccount<'info>>,

    #[account(mut)]
    /// CHECK: User's token x account
    pub user_token_x: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: User's token y account
    pub user_token_y: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Reserve account of token X
    pub reserve_x: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Reserve account of token Y
    pub reserve_y: UncheckedAccount<'info>,

    /// CHECK: Mint account of token X
    pub token_x_mint: UncheckedAccount<'info>,
    /// CHECK: Mint account of token Y
    pub token_y_mint: UncheckedAccount<'info>,

    #[account(mut)]
    /// CHECK: Bin array lower account
    pub bin_array_lower: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: Bin array upper account
    pub bin_array_upper: UncheckedAccount<'info>,

    /// CHECK: User who is withdrawing from DLMM pool
    pub sender: Signer<'info>,

    #[account(address = dlmm::ID)]
    /// CHECK: DLMM program
    pub dlmm_program: UncheckedAccount<'info>,

    /// CHECK: DLMM program event authority for event CPI
    pub event_authority: UncheckedAccount<'info>,

    /// CHECK: Token program of mint X
    pub token_x_program: UncheckedAccount<'info>,
    /// CHECK: Token program of mint Y
    pub token_y_program: UncheckedAccount<'info>,
}

pub fn dlmm_to_gamma(
    ctx: Context<DlmmToGamma>,
    bin_liquidity_reduction: Vec<BinLiquidityReduction>,
) -> Result<()> {
    // Construct the instruction data for the CPI call
    let modify_liquidity_instruction = RemoveLiquidity {
        bin_liquidity_removal: bin_liquidity_reduction,
    };

    // Serialize the instruction data
    let instruction_data = bincode::serialize(&modify_liquidity_instruction)
    .map_err(|_| ProgramError::InvalidInstructionData)?;


    // Prepare the list of AccountMetas
    let mut account_metas = vec![
        AccountMeta::new(ctx.accounts.position.key(), false),
        AccountMeta::new(ctx.accounts.lb_pair.key(), false),
    ];

    // Include the optional bin_array_bitmap_extension account if it's provided
    if let Some(bin_array_bitmap_extension) = &ctx.accounts.bin_array_bitmap_extension {
        account_metas.push(AccountMeta::new_readonly(
            bin_array_bitmap_extension.key(),
            false,
        ));
    }

    account_metas.extend(vec![
        AccountMeta::new(ctx.accounts.user_token_x.key(), false),
        AccountMeta::new(ctx.accounts.user_token_y.key(), false),
        AccountMeta::new(ctx.accounts.reserve_x.key(), false),
        AccountMeta::new(ctx.accounts.reserve_y.key(), false),
        AccountMeta::new_readonly(ctx.accounts.token_x_mint.key(), false),
        AccountMeta::new_readonly(ctx.accounts.token_y_mint.key(), false),
        AccountMeta::new(ctx.accounts.bin_array_lower.key(), false),
        AccountMeta::new(ctx.accounts.bin_array_upper.key(), false),
        AccountMeta::new_readonly(ctx.accounts.sender.key(), true),
        AccountMeta::new_readonly(ctx.accounts.token_x_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.token_y_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.event_authority.key(), false),
    ]);

    // Construct the instruction
    let ix = Instruction {
        program_id: ctx.accounts.dlmm_program.key(),
        accounts: account_metas,
        data: instruction_data,
    };

    // Prepare the list of AccountInfos
    let mut account_infos = vec![
        ctx.accounts.position.to_account_info(),
        ctx.accounts.lb_pair.to_account_info(),
    ];

    if let Some(bin_array_bitmap_extension) = &ctx.accounts.bin_array_bitmap_extension {
        account_infos.push(bin_array_bitmap_extension.to_account_info());
    }

    account_infos.extend(vec![
        ctx.accounts.user_token_x.to_account_info(),
        ctx.accounts.user_token_y.to_account_info(),
        ctx.accounts.reserve_x.to_account_info(),
        ctx.accounts.reserve_y.to_account_info(),
        ctx.accounts.token_x_mint.to_account_info(),
        ctx.accounts.token_y_mint.to_account_info(),
        ctx.accounts.bin_array_lower.to_account_info(),
        ctx.accounts.bin_array_upper.to_account_info(),
        ctx.accounts.sender.to_account_info(),
        ctx.accounts.token_x_program.to_account_info(),
        ctx.accounts.token_y_program.to_account_info(),
        ctx.accounts.event_authority.to_account_info(),
        ctx.accounts.dlmm_program.to_account_info(),
    ]);

    // Invoke the CPI call using the low-level `invoke` function
    invoke(
        &ix,
        &account_infos,
    )?;

    // Proceed to deposit the withdrawn tokens into the Gamma pool as needed
    // You can add the logic for depositing into the Gamma pool here

    Ok(())
}
