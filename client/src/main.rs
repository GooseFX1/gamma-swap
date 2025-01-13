#![allow(dead_code)]
use anchor_client::{Client, Cluster};
use anyhow::{format_err, Result};
use arrayref::array_ref;
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use solana_client::{rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    signature::{Keypair, Signature, Signer},
    transaction::Transaction,
};
use solana_transaction_status::UiTransactionEncoding;
use std::env;
use std::rc::Rc;
use std::str::FromStr;
use std::time::SystemTime;

mod instructions;
use instructions::amm_instructions::*;
use instructions::events_instructions_parse::*;
use instructions::rpc::*;
use instructions::token_instructions::*;
use instructions::utils::*;
use spl_token_2022::{
    extension::StateWithExtensionsMut,
    state::{Account, Mint},
};
mod test_swaps;
use test_swaps::run_swap_test;

#[derive(Clone, Debug, PartialEq)]
pub struct ClientConfig {
    http_url: String,
    ws_url: String,
    payer_path: String,
    admin_path: String,
    gamma_program: Pubkey,
    slippage: f64,
}

fn load_cfg(opts: &Opts) -> Result<ClientConfig, Box<dyn std::error::Error>> {
    dotenv().ok();

    let http_url = opts
        .http_url
        .clone()
        .unwrap_or_else(|| env::var("HTTP_URL").expect("HTTP_URL must be set"));
    let ws_url = opts
        .ws_url
        .clone()
        .unwrap_or_else(|| env::var("WS_URL").expect("WS_URL must be set"));
    let payer_path = opts
        .payer_path
        .clone()
        .unwrap_or_else(|| env::var("PAYER_PATH").expect("PAYER_PATH must be set"));
    let admin_path = opts
        .admin_path
        .clone()
        .unwrap_or_else(|| env::var("ADMIN_PATH").expect("ADMIN_PATH must be set"));
    let gamma_program_str = opts
        .gamma_program
        .clone()
        .unwrap_or_else(|| env::var("GAMMA_PROGRAM").expect("GAMMA_PROGRAM must be set"));
    let slippage = opts.slippage.unwrap_or_else(|| {
        env::var("SLIPPAGE")
            .expect("SLIPPAGE must be set")
            .parse::<f64>()
            .expect("SLIPPAGE must be a valid float")
    });

    let gamma_program =
        Pubkey::from_str(&gamma_program_str).map_err(|_| "Invalid GAMMA_PROGRAM pubkey")?;

    Ok(ClientConfig {
        http_url,
        ws_url,
        payer_path,
        admin_path,
        gamma_program,
        slippage,
    })
}

fn read_keypair_file(s: &str) -> Result<Keypair> {
    solana_sdk::signature::read_keypair_file(s)
        .map_err(|_| format_err!("failed to read keypair from {}", s))
}
#[derive(Parser, Debug)]
#[clap(name = "gamma-cli")]
pub struct Opts {
    #[clap(long, env = "HTTP_URL")]
    http_url: Option<String>,

    #[clap(long, env = "WS_URL")]
    ws_url: Option<String>,

    #[clap(long, env = "PAYER_PATH")]
    payer_path: Option<String>,

    #[clap(long, env = "ADMIN_PATH")]
    admin_path: Option<String>,

    #[clap(long, env = "GAMMA_PROGRAM")]
    gamma_program: Option<String>,

    #[clap(long, env = "SLIPPAGE")]
    slippage: Option<f64>,

    #[clap(subcommand)]
    command: GammaCommands,
}

#[derive(Debug, Subcommand, Clone)]
pub enum GammaCommands {
    CreateConfig {
        #[clap(short, long)]
        amm_index: u16,
        #[clap(short, long)]
        trade_fee_rate: u64,
        #[clap(short, long)]
        protocol_fee_rate: u64,
        #[clap(short, long)]
        fund_fee_rate: u64,
        #[clap(short, long)]
        create_pool_fee: u64,
    },
    CreateReferralProject {
        amm_config: Pubkey,
        referral_program: Pubkey,
        name: String,
        default_share_bps: u16,
    },
    InitializePool {
        mint0: Pubkey,
        mint1: Pubkey,
        init_amount_0: u64,
        init_amount_1: u64,
        #[clap(short, long, default_value_t = 0)]
        open_time: u64,
    },
    InitUserPoolLiquidity {
        pool_id: Pubkey,
    },
    Deposit {
        pool_id: Pubkey,
        lp_token_amount: u64,
    },
    Withdraw {
        pool_id: Pubkey,
        lp_token_amount: u64,
    },
    SwapBaseIn {
        pool_id: Pubkey,
        user_input_token: Pubkey,
        user_input_amount: u64,
    },
    SwapBaseOut {
        pool_id: Pubkey,
        user_input_token: Pubkey,
        amount_out_less_fee: u64,
    },
    DecodeInstruction {
        instr_hex_data: String,
    },
    DecodeEvent {
        log_event: String,
    },
    DecodeTxLog {
        tx_id: String,
    },
    TestSwaps {
        user_keypair: String,
    },
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    let pool_config = load_cfg(&opts).unwrap();
    // cluster params.
    let payer = read_keypair_file(&pool_config.payer_path)?;
    // solana rpc client
    let rpc_client = RpcClient::new(pool_config.http_url.to_string());

    // anchor client.
    let anchor_config = pool_config.clone();
    let url = Cluster::Custom(anchor_config.http_url, anchor_config.ws_url);
    let wallet = read_keypair_file(&pool_config.payer_path)?;
    let anchor_client = Client::new(url, Rc::new(wallet));
    let program = anchor_client.program(pool_config.gamma_program)?;

    let opts = Opts::parse();
    match opts.command {
        GammaCommands::CreateConfig {
            amm_index,
            trade_fee_rate,
            protocol_fee_rate,
            fund_fee_rate,
            create_pool_fee,
        } => {
            let mut instructions = Vec::new();

            let create_config_instr = create_config_instr(
                &pool_config,
                amm_index,
                trade_fee_rate,
                protocol_fee_rate,
                fund_fee_rate,
                create_pool_fee,
            )?;
            instructions.extend(create_config_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::CreateReferralProject {
            name,
            default_share_bps,
            referral_program,
            amm_config,
        } => {
            let instruction = create_referral_project_instr(
                &pool_config,
                payer.pubkey(),
                amm_config,
                name,
                default_share_bps,
                referral_program,
            );

            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &vec![&payer],
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::InitializePool {
            mint0,
            mint1,
            init_amount_0,
            init_amount_1,
            open_time,
        } => {
            let (mint0, mint1, init_amount_0, init_amount_1) = if mint0 > mint1 {
                (mint1, mint0, init_amount_1, init_amount_0)
            } else {
                (mint0, mint1, init_amount_0, init_amount_1)
            };
            let load_pubkeys = vec![mint0, mint1];
            let rsps = rpc_client.get_multiple_accounts(&load_pubkeys)?;
            let token_0_program = rsps[0].clone().unwrap().owner;
            let token_1_program = rsps[1].clone().unwrap().owner;

            let initialize_pool_instr = initialize_pool_instr(
                &pool_config,
                mint0,
                mint1,
                token_0_program,
                token_1_program,
                spl_associated_token_account::get_associated_token_address(&payer.pubkey(), &mint0),
                spl_associated_token_account::get_associated_token_address(&payer.pubkey(), &mint1),
                gamma::create_pool_fee_reveiver::id(),
                init_amount_0,
                init_amount_1,
                open_time,
            )?;

            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &initialize_pool_instr,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::InitUserPoolLiquidity { pool_id } => {
            let init_user_pool_liquidity_instr =
                init_user_pool_liquidity_instr(&pool_config, pool_id)?;
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let mut instructions = Vec::new();
            instructions.extend(init_user_pool_liquidity_instr);
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::Deposit {
            pool_id,
            lp_token_amount,
        } => {
            let pool_state: gamma::states::PoolState = program.account(pool_id)?;
            // load account
            let load_pubkeys = vec![pool_state.token_0_vault, pool_state.token_1_vault];
            let rsps = rpc_client.get_multiple_accounts(&load_pubkeys)?;
            let [token_0_vault_account, token_1_vault_account] = array_ref![rsps, 0, 2];
            // docode account
            let mut token_0_vault_data = token_0_vault_account.clone().unwrap().data;
            let mut token_1_vault_data = token_1_vault_account.clone().unwrap().data;
            let _token_0_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_0_vault_data)?;
            let _token_1_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_1_vault_data)?;

            let (total_token_0_amount, total_token_1_amount) =
                pool_state.vault_amount_without_fee()?;
            // calculate amount
            let results = gamma::curve::CurveCalculator::lp_tokens_to_trading_tokens(
                u128::from(lp_token_amount),
                u128::from(pool_state.lp_supply),
                u128::from(total_token_0_amount),
                u128::from(total_token_1_amount),
                gamma::curve::RoundDirection::Ceiling,
            )
            .ok_or(gamma::error::GammaError::ZeroTradingTokens)
            .unwrap();
            println!(
                "amount_0:{}, amount_1:{}, lp_token_amount:{}",
                results.token_0_amount, results.token_1_amount, lp_token_amount
            );
            // calc with slippage
            let amount_0_with_slippage =
                amount_with_slippage(results.token_0_amount as u64, pool_config.slippage, true);
            let amount_1_with_slippage =
                amount_with_slippage(results.token_1_amount as u64, pool_config.slippage, true);
            // calc with transfer_fee
            let transfer_fee = get_pool_mints_inverse_fee(
                &rpc_client,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                amount_0_with_slippage,
                amount_1_with_slippage,
            );
            println!(
                "transfer_fee_0:{}, transfer_fee_1:{}",
                transfer_fee.0.transfer_fee, transfer_fee.1.transfer_fee
            );
            let amount_0_max = (amount_0_with_slippage as u64)
                .checked_add(transfer_fee.0.transfer_fee)
                .unwrap();
            let amount_1_max = (amount_1_with_slippage as u64)
                .checked_add(transfer_fee.1.transfer_fee)
                .unwrap();
            println!(
                "amount_0_max:{}, amount_1_max:{}",
                amount_0_max, amount_1_max
            );
            let mut instructions = Vec::new();
            // let create_user_lp_token_instr = create_ata_token_account_instr(
            //     &pool_config,
            //     spl_token::id(),
            //     &pool_state.lp_mint,
            //     &payer.pubkey(),
            // )?;
            // instructions.extend(create_user_lp_token_instr);
            let user_token_0_ata = spl_associated_token_account::get_associated_token_address(
                &payer.pubkey(),
                &pool_state.token_0_mint,
            );
            let user_token_1_ata = spl_associated_token_account::get_associated_token_address(
                &payer.pubkey(),
                &pool_state.token_1_mint,
            );
            let deposit_instr = deposit_instr(
                &pool_config,
                pool_id,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                // pool_state.lp_mint,
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                user_token_0_ata,
                user_token_1_ata,
                // spl_associated_token_account::get_associated_token_address(
                //     &payer.pubkey(),
                //     &pool_state.lp_mint,
                // ),
                lp_token_amount,
                amount_0_max,
                amount_1_max,
            )?;
            instructions.extend(deposit_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::Withdraw {
            pool_id,
            lp_token_amount,
        } => {
            let pool_state: gamma::states::PoolState = program.account(pool_id)?;
            // load account
            let load_pubkeys = vec![pool_state.token_0_vault, pool_state.token_1_vault];
            let rsps = rpc_client.get_multiple_accounts(&load_pubkeys)?;
            let [token_0_vault_account, token_1_vault_account] = array_ref![rsps, 0, 2];
            // docode account
            let mut token_0_vault_data = token_0_vault_account.clone().unwrap().data;
            let mut token_1_vault_data = token_1_vault_account.clone().unwrap().data;
            let _token_0_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_0_vault_data)?;
            let _token_1_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_1_vault_data)?;

            let (total_token_0_amount, total_token_1_amount) =
                pool_state.vault_amount_without_fee()?;
            // calculate amount
            let results = gamma::curve::CurveCalculator::lp_tokens_to_trading_tokens(
                u128::from(lp_token_amount),
                u128::from(pool_state.lp_supply),
                u128::from(total_token_0_amount),
                u128::from(total_token_1_amount),
                gamma::curve::RoundDirection::Ceiling,
            )
            .ok_or(gamma::error::GammaError::ZeroTradingTokens)
            .unwrap();
            println!(
                "amount_0:{}, amount_1:{}, lp_token_amount:{}",
                results.token_0_amount, results.token_1_amount, lp_token_amount
            );

            // calc with slippage
            let amount_0_with_slippage =
                amount_with_slippage(results.token_0_amount as u64, pool_config.slippage, false);
            let amount_1_with_slippage =
                amount_with_slippage(results.token_1_amount as u64, pool_config.slippage, false);

            let transfer_fee = get_pool_mints_transfer_fee(
                &rpc_client,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                amount_0_with_slippage,
                amount_1_with_slippage,
            );
            println!(
                "transfer_fee_0:{}, transfer_fee_1:{}",
                transfer_fee.0.transfer_fee, transfer_fee.1.transfer_fee
            );
            let amount_0_min = amount_0_with_slippage
                .checked_sub(transfer_fee.0.transfer_fee)
                .unwrap();
            let amount_1_min = amount_1_with_slippage
                .checked_sub(transfer_fee.1.transfer_fee)
                .unwrap();
            println!(
                "amount_0_min:{}, amount_1_min:{}",
                amount_0_min, amount_1_min
            );
            let mut instructions = Vec::new();
            let create_user_token_0_instr = create_ata_token_account_instr(
                &pool_config,
                spl_token::id(),
                &pool_state.token_0_mint,
                &payer.pubkey(),
            )?;
            instructions.extend(create_user_token_0_instr);
            let create_user_token_1_instr = create_ata_token_account_instr(
                &pool_config,
                spl_token::id(),
                &pool_state.token_1_mint,
                &payer.pubkey(),
            )?;
            instructions.extend(create_user_token_1_instr);
            let withdraw_instr = withdraw_instr(
                &pool_config,
                pool_id,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                // pool_state.lp_mint,
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                spl_associated_token_account::get_associated_token_address(
                    &payer.pubkey(),
                    &pool_state.token_0_mint,
                ),
                spl_associated_token_account::get_associated_token_address(
                    &payer.pubkey(),
                    &pool_state.token_1_mint,
                ),
                lp_token_amount,
                amount_0_min,
                amount_1_min,
            )?;
            instructions.extend(withdraw_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::SwapBaseIn {
            pool_id,
            user_input_token,
            user_input_amount,
        } => {
            let pool_state: gamma::states::PoolState = program.account(pool_id)?;
            // load account
            let load_pubkeys = vec![
                pool_state.amm_config,
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                user_input_token,
            ];
            let rsps = rpc_client.get_multiple_accounts(&load_pubkeys)?;
            let epoch = rpc_client.get_epoch_info().unwrap().epoch;
            let [amm_config_account, token_0_vault_account, token_1_vault_account, token_0_mint_account, token_1_mint_account, user_input_token_account] =
                array_ref![rsps, 0, 6];
            // docode account
            let mut token_0_vault_data = token_0_vault_account.clone().unwrap().data;
            let mut token_1_vault_data = token_1_vault_account.clone().unwrap().data;
            let mut token_0_mint_data = token_0_mint_account.clone().unwrap().data;
            let mut token_1_mint_data = token_1_mint_account.clone().unwrap().data;
            let mut user_input_token_data = user_input_token_account.clone().unwrap().data;
            let amm_config_state = deserialize_anchor_account::<gamma::states::AmmConfig>(
                amm_config_account.as_ref().unwrap(),
            )?;
            let token_0_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_0_vault_data)?;
            let _token_1_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_1_vault_data)?;
            let token_0_mint_info = StateWithExtensionsMut::<Mint>::unpack(&mut token_0_mint_data)?;
            let token_1_mint_info = StateWithExtensionsMut::<Mint>::unpack(&mut token_1_mint_data)?;
            let user_input_token_info =
                StateWithExtensionsMut::<Account>::unpack(&mut user_input_token_data)?;

            let (total_token_0_amount, total_token_1_amount) =
                pool_state.vault_amount_without_fee()?;

            let (
                trade_direction,
                total_input_token_amount,
                total_output_token_amount,
                user_input_token,
                user_output_token,
                input_vault,
                output_vault,
                input_token_mint,
                output_token_mint,
                input_token_program,
                output_token_program,
                transfer_fee,
            ) = if user_input_token_info.base.mint == token_0_vault_info.base.mint {
                (
                    gamma::curve::TradeDirection::ZeroForOne,
                    total_token_0_amount,
                    total_token_1_amount,
                    user_input_token,
                    spl_associated_token_account::get_associated_token_address(
                        &payer.pubkey(),
                        &pool_state.token_1_mint,
                    ),
                    pool_state.token_0_vault,
                    pool_state.token_1_vault,
                    pool_state.token_0_mint,
                    pool_state.token_1_mint,
                    pool_state.token_0_program,
                    pool_state.token_1_program,
                    get_transfer_fee(&token_0_mint_info, epoch, user_input_amount),
                )
            } else {
                (
                    gamma::curve::TradeDirection::OneForZero,
                    total_token_1_amount,
                    total_token_0_amount,
                    user_input_token,
                    spl_associated_token_account::get_associated_token_address(
                        &payer.pubkey(),
                        &pool_state.token_0_mint,
                    ),
                    pool_state.token_1_vault,
                    pool_state.token_0_vault,
                    pool_state.token_1_mint,
                    pool_state.token_0_mint,
                    pool_state.token_1_program,
                    pool_state.token_0_program,
                    get_transfer_fee(&token_1_mint_info, epoch, user_input_amount),
                )
            };
            // Take transfer fees into account for actual amount transferred in
            let actual_amount_in = user_input_amount.saturating_sub(transfer_fee);

            let current_unix_timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // here we make a RPC call again, we can optimize this later by making it part of `get_multiple_accounts` call.
            let observation: gamma::states::ObservationState =
                program.account(pool_state.observation_key)?;

            let result = gamma::curve::CurveCalculator::swap_base_input(
                u128::from(actual_amount_in),
                u128::from(total_input_token_amount),
                u128::from(total_output_token_amount),
                &amm_config_state,
                &pool_state,
                current_unix_timestamp,
                &observation,
                false,
            )?;

            let amount_out = u64::try_from(result.destination_amount_swapped).unwrap();
            let transfer_fee = match trade_direction {
                gamma::curve::TradeDirection::ZeroForOne => {
                    get_transfer_fee(&token_1_mint_info, epoch, amount_out)
                }
                gamma::curve::TradeDirection::OneForZero => {
                    get_transfer_fee(&token_0_mint_info, epoch, amount_out)
                }
            };
            let amount_received = amount_out.checked_sub(transfer_fee).unwrap();
            // calc mint out amount with slippage
            let minimum_amount_out =
                amount_with_slippage(amount_received, pool_config.slippage, false);

            let mut instructions = Vec::new();
            let create_user_output_token_instr = create_ata_token_account_instr(
                &pool_config,
                spl_token::id(),
                &output_token_mint,
                &payer.pubkey(),
            )?;
            instructions.extend(create_user_output_token_instr);
            let swap_base_in_instr = swap_base_input_instr(
                &pool_config,
                pool_id,
                pool_state.amm_config,
                pool_state.observation_key,
                user_input_token,
                user_output_token,
                input_vault,
                output_vault,
                input_token_mint,
                output_token_mint,
                input_token_program,
                output_token_program,
                user_input_amount,
                minimum_amount_out,
            )?;
            instructions.extend(swap_base_in_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::SwapBaseOut {
            pool_id,
            user_input_token,
            amount_out_less_fee,
        } => {
            let pool_state: gamma::states::PoolState = program.account(pool_id)?;
            // load account
            let load_pubkeys = vec![
                pool_state.amm_config,
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                user_input_token,
            ];
            let rsps = rpc_client.get_multiple_accounts(&load_pubkeys)?;
            let epoch = rpc_client.get_epoch_info().unwrap().epoch;
            let [amm_config_account, token_0_vault_account, token_1_vault_account, token_0_mint_account, token_1_mint_account, user_input_token_account] =
                array_ref![rsps, 0, 6];
            // decode account
            let mut token_0_vault_data = token_0_vault_account.clone().unwrap().data;
            let mut token_1_vault_data = token_1_vault_account.clone().unwrap().data;
            let mut token_0_mint_data = token_0_mint_account.clone().unwrap().data;
            let mut token_1_mint_data = token_1_mint_account.clone().unwrap().data;
            let mut user_input_token_data = user_input_token_account.clone().unwrap().data;
            let amm_config_state = deserialize_anchor_account::<gamma::states::AmmConfig>(
                amm_config_account.as_ref().unwrap(),
            )?;
            let token_0_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_0_vault_data)?;
            let _token_1_vault_info =
                StateWithExtensionsMut::<Account>::unpack(&mut token_1_vault_data)?;
            let token_0_mint_info = StateWithExtensionsMut::<Mint>::unpack(&mut token_0_mint_data)?;
            let token_1_mint_info = StateWithExtensionsMut::<Mint>::unpack(&mut token_1_mint_data)?;
            let user_input_token_info =
                StateWithExtensionsMut::<Account>::unpack(&mut user_input_token_data)?;

            let (total_token_0_amount, total_token_1_amount) =
                pool_state.vault_amount_without_fee()?;

            let (
                trade_direction,
                total_input_token_amount,
                total_output_token_amount,
                user_input_token,
                user_output_token,
                input_vault,
                output_vault,
                input_token_mint,
                output_token_mint,
                input_token_program,
                output_token_program,
                out_transfer_fee,
            ) = if user_input_token_info.base.mint == token_0_vault_info.base.mint {
                (
                    gamma::curve::TradeDirection::ZeroForOne,
                    total_token_0_amount,
                    total_token_1_amount,
                    user_input_token,
                    spl_associated_token_account::get_associated_token_address(
                        &payer.pubkey(),
                        &pool_state.token_1_mint,
                    ),
                    pool_state.token_0_vault,
                    pool_state.token_1_vault,
                    pool_state.token_0_mint,
                    pool_state.token_1_mint,
                    pool_state.token_0_program,
                    pool_state.token_1_program,
                    get_transfer_inverse_fee(&token_1_mint_info, epoch, amount_out_less_fee),
                )
            } else {
                (
                    gamma::curve::TradeDirection::OneForZero,
                    total_token_1_amount,
                    total_token_0_amount,
                    user_input_token,
                    spl_associated_token_account::get_associated_token_address(
                        &payer.pubkey(),
                        &pool_state.token_0_mint,
                    ),
                    pool_state.token_1_vault,
                    pool_state.token_0_vault,
                    pool_state.token_1_mint,
                    pool_state.token_0_mint,
                    pool_state.token_1_program,
                    pool_state.token_0_program,
                    get_transfer_inverse_fee(&token_0_mint_info, epoch, amount_out_less_fee),
                )
            };
            let actual_amount_out = amount_out_less_fee.checked_add(out_transfer_fee).unwrap();
            let current_unix_timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // here we make a RPC call again, we can optimize this later by making it part of `get_multiple_accounts` call.
            let observation: gamma::states::ObservationState =
                program.account(pool_state.observation_key)?;

            let result = gamma::curve::CurveCalculator::swap_base_output(
                u128::from(actual_amount_out),
                u128::from(total_input_token_amount),
                u128::from(total_output_token_amount),
                &amm_config_state,
                &pool_state,
                current_unix_timestamp,
                &observation,
                false,
            )?;

            let source_amount_swapped = u64::try_from(result.source_amount_swapped).unwrap();
            let amount_in_transfer_fee = match trade_direction {
                gamma::curve::TradeDirection::ZeroForOne => {
                    get_transfer_inverse_fee(&token_0_mint_info, epoch, source_amount_swapped)
                }
                gamma::curve::TradeDirection::OneForZero => {
                    get_transfer_inverse_fee(&token_1_mint_info, epoch, source_amount_swapped)
                }
            };

            let input_transfer_amount = source_amount_swapped
                .checked_add(amount_in_transfer_fee)
                .unwrap();
            // calc max in with slippage
            let max_amount_in =
                amount_with_slippage(input_transfer_amount, pool_config.slippage, true);
            let mut instructions = Vec::new();
            let create_user_output_token_instr = create_ata_token_account_instr(
                &pool_config,
                spl_token::id(),
                &output_token_mint,
                &payer.pubkey(),
            )?;
            instructions.extend(create_user_output_token_instr);
            let swap_base_in_instr = swap_base_output_instr(
                &pool_config,
                pool_id,
                pool_state.amm_config,
                pool_state.observation_key,
                user_input_token,
                user_output_token,
                input_vault,
                output_vault,
                input_token_mint,
                output_token_mint,
                input_token_program,
                output_token_program,
                max_amount_in,
                amount_out_less_fee,
            )?;
            instructions.extend(swap_base_in_instr);
            let signers = vec![&payer];
            let recent_hash = rpc_client.get_latest_blockhash()?;
            let txn = Transaction::new_signed_with_payer(
                &instructions,
                Some(&payer.pubkey()),
                &signers,
                recent_hash,
            );
            let signature = send_txn(&rpc_client, &txn, true)?;
            println!("{}", signature);
        }
        GammaCommands::DecodeInstruction { instr_hex_data } => {
            handle_program_instruction(&instr_hex_data, InstructionDecodeType::BaseHex)?;
        }
        GammaCommands::DecodeEvent { log_event } => {
            handle_program_log(&pool_config.gamma_program.to_string(), &log_event, false)?;
        }
        GammaCommands::DecodeTxLog { tx_id } => {
            let signature = Signature::from_str(&tx_id)?;
            let tx = rpc_client.get_transaction_with_config(
                &signature,
                RpcTransactionConfig {
                    encoding: Some(UiTransactionEncoding::Json),
                    commitment: Some(CommitmentConfig::confirmed()),
                    max_supported_transaction_version: Some(0),
                },
            )?;
            let transaction = tx.transaction;
            // get meta
            let meta = if transaction.meta.is_some() {
                transaction.meta
            } else {
                None
            };
            // get encoded_transaction
            let encoded_transaction = transaction.transaction;
            // decode instruction data
            parse_program_instruction(
                &pool_config.gamma_program.to_string(),
                encoded_transaction,
                meta.clone(),
            )?;
            // decode logs
            parse_program_event(&pool_config.gamma_program.to_string(), meta.clone())?;
        }
        GammaCommands::TestSwaps { user_keypair } => {
            run_swap_test(&pool_config, user_keypair)?;
        }
    }
    Ok(())
}
