use std::str::FromStr;
use solana_sdk::pubkey::Pubkey;
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::read_keypair_file;
use solana_sdk::signer::Signer;
use spl_associated_token_account::get_associated_token_address;
use spl_token_2022::state::{Account as TokenAccount, Mint};
use spl_token_2022::extension::StateWithExtensionsMut;
use anyhow::Result;
use arrayref::array_ref;
use crate::{deserialize_anchor_account, ClientConfig};
use std::process::Command;

const POOL_ID: &str = "Fc9eSn5QpAiPAmT3UFpDd6ExTeQ4MP7X8R3qcfUCFG1T";
const N4_MINT: &str = "N4CdHcZYMj7DufSu89m1gi3RFxt8NiJQ9PmfNg8kc8P";
const N5_MINT: &str = "N5Y2m9HSPDBr8ft6UVWL4vLoaBfqPYawxhM1uEMx5Gk";
const USER_1_N4_ACCOUNT: &str = "6yzVXwDCu27uWMrU78EJBUHhKBfcbs71WfZeURPhsNwj";
const USER_1_N5_ACCOUNT: &str = "GJWGyu7KoRjhMDVDx7YZr6nbVHPcUNMHEbLvKtPTGZa6";
const USER_2_N4_ACCOUNT: &str = "E4ZnAhLhBNcUWcU4W9En2YxuPzdd8SA1GYX4V65oeB4s";
const USER_2_N5_ACCOUNT: &str = "3vD6Ga7CbxdfduRmDZZqSBoWkhypNNZf9coxKvrJag9v";

#[derive(Debug)]
struct TestPoolState {
    token_0_vault: Pubkey,
    token_1_vault: Pubkey,
    protocol_fees_token_0: u64,
    protocol_fees_token_1: u64,
    fund_fees_token_0: u64,
    fund_fees_token_1: u64,
    cumulative_trade_fees_token_0: u128,
    cumulative_trade_fees_token_1: u128,
    cumulative_volume_token_0: u128,
    cumulative_volume_token_1: u128,
    lp_supply: u64,
}

fn get_token_balance(client: &RpcClient, account: &Pubkey) -> Result<u64> {
    let load_pubkey = vec![account.clone()];
    let rsps = client.get_multiple_accounts(&load_pubkey)?;
    let [account_info] = array_ref![rsps, 0, 1];
    let mut account_data = account_info.clone().unwrap().data;
    let token_account = StateWithExtensionsMut::<TokenAccount>::unpack(&mut account_data)?;
    Ok(token_account.base.amount)
}

fn get_token_mint(client: &RpcClient, account: &Pubkey) -> Result<Pubkey> {
    let load_pubkey = vec![account.clone()];
    let rsps = client.get_multiple_accounts(&load_pubkey)?;
    let [account_info] = array_ref![rsps, 0, 1];
    let mut account_data = account_info.clone().unwrap().data;
    let token_account = StateWithExtensionsMut::<TokenAccount>::unpack(&mut account_data)?;
    Ok(token_account.base.mint)
}

fn get_pool_state(client: &RpcClient, pool_id: &Pubkey) -> Result<TestPoolState> {
    let load_pubkeys = vec![pool_id.clone()];
    let rsps = client.get_multiple_accounts(&load_pubkeys)?;
    let [pool_account] = array_ref![rsps, 0, 1];
    let pool_state = deserialize_anchor_account::<gamma::states::PoolState>(&pool_account.as_ref().unwrap())?;
    
    Ok(TestPoolState {
        token_0_vault: pool_state.token_0_vault,
        token_1_vault: pool_state.token_1_vault,
        protocol_fees_token_0: pool_state.protocol_fees_token_0,
        protocol_fees_token_1: pool_state.protocol_fees_token_1,
        fund_fees_token_0: pool_state.fund_fees_token_0,
        fund_fees_token_1: pool_state.fund_fees_token_1,
        cumulative_trade_fees_token_0: pool_state.cumulative_trade_fees_token_0,
        cumulative_trade_fees_token_1: pool_state.cumulative_trade_fees_token_1,
        cumulative_volume_token_0: pool_state.cumulative_volume_token_0,
        cumulative_volume_token_1: pool_state.cumulative_volume_token_1,
        lp_supply: pool_state.lp_supply,
    })
}

fn calculate_price(amount_0: u64, amount_1: u64, decimals_0: u8, decimals_1: u8) -> f64 {
    let amount_0 = amount_0 as f64 / 10f64.powi(decimals_0 as i32);
    let amount_1 = amount_1 as f64 / 10f64.powi(decimals_1 as i32);
    amount_1 / amount_0
}

fn calculate_k(amount_0: u64, amount_1: u64) -> u128 {
    (amount_0 as u128) * (amount_1 as u128)
}

fn calculate_lp_amount(amount_0: u64, amount_1: u64) -> f64 {
    ((amount_0 as f64) * (amount_1 as f64)).sqrt()
}

pub fn run_swap_test(config: &ClientConfig, user_keypair_path: String) -> Result<()> {
    let rpc_client = RpcClient::new(config.http_url.clone());
    let pool_id = Pubkey::from_str(POOL_ID)?;
    let n4_mint = Pubkey::from_str(N4_MINT)?;
    let n5_mint = Pubkey::from_str(N5_MINT)?;
    let user_1_n4_account = Pubkey::from_str(USER_1_N4_ACCOUNT)?;
    let user_1_n5_account = Pubkey::from_str(USER_1_N5_ACCOUNT)?;
    let user_2_n4_account = Pubkey::from_str(USER_2_N4_ACCOUNT)?;
    let user_2_n5_account = Pubkey::from_str(USER_2_N5_ACCOUNT)?;

    let load_pubkeys = vec![n4_mint, n5_mint];
    let rsps = rpc_client.get_multiple_accounts(&load_pubkeys)?;
    let [n4_mint_account, n5_mint_account] = array_ref![rsps, 0, 2];
    let mut n4_mint_data = n4_mint_account.clone().unwrap().data;
    let mut n5_mint_data = n5_mint_account.clone().unwrap().data;
    let n4_decimals = StateWithExtensionsMut::<Mint>::unpack(&mut n4_mint_data)?.base.decimals;
    let n5_decimals = StateWithExtensionsMut::<Mint>::unpack(&mut n5_mint_data)?.base.decimals;

    let swap_amounts_n4: Vec<u64> = vec![1_000_000_000, 2_000_000_000, 3_000_000_000];
    let swap_amounts_n5: Vec<u64> = vec![150_000_000, 300_000_000, 450_000_000];

    println!("Initial state:");
    print_state(&rpc_client, &pool_id, &user_1_n4_account, &user_1_n5_account, n4_decimals, n5_decimals, "User 1")?;
    print_state(&rpc_client, &pool_id, &user_2_n4_account, &user_2_n5_account, n4_decimals, n5_decimals, "User 2")?;
    let user_keypair = read_keypair_file(user_keypair_path.clone()).unwrap();
    let user_pubkey = user_keypair.pubkey();
    let user_token_0_account = get_associated_token_address(&user_pubkey, &n4_mint);
    let user_token_1_account = get_associated_token_address(&user_pubkey, &n5_mint);
    for (i, &amount) in swap_amounts_n4.iter().enumerate() {
        println!("\n--- Swap {} User 1: N4 to N5 ---", i + 1);

        // User 1: N4 to N5 swap
        execute_swap(&pool_id, &user_token_0_account, amount, "User 1: N4 to N5")?;
        print_state(&rpc_client, &pool_id, &user_token_0_account, &user_token_1_account, n4_decimals, n5_decimals, "User 1")?;
    }
    
    for (i, &amount) in swap_amounts_n5.iter().enumerate() {
        println!("\n--- Swap {} User 1: N5 to N4 ---", i + 1);

        // User 1: N5 to N4 swap
        execute_swap(&pool_id, &user_token_1_account, amount, "User 1: N5 to N4")?;
        print_state(&rpc_client, &pool_id, &user_token_0_account, &user_token_1_account, n4_decimals, n5_decimals, "User 1")?;
    }

    Ok(())
}

fn execute_swap(
    pool_id: &Pubkey,
    input_account: &Pubkey,
    amount: u64,
    description: &str,
) -> Result<()> {
    println!("\nExecuting {} swap with amount: {}", description, amount);
    let output = Command::new("gamma-cli")
        .arg("swap-base-in")
        .arg(pool_id.to_string())
        .arg(input_account.to_string())
        .arg(amount.to_string())
        .output()?;

    println!("Swap output: {}", String::from_utf8_lossy(&output.stdout));

    if !output.status.success() {
        println!("Swap error: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

fn print_state(
    rpc_client: &RpcClient,
    pool_id: &Pubkey,
    user_n4_account: &Pubkey,
    user_n5_account: &Pubkey,
    n4_decimals: u8,
    n5_decimals: u8,
    user_description: &str,
) -> Result<()> {
    let pool_state = get_pool_state(rpc_client, pool_id)?;
    let token_0_balance = get_token_balance(rpc_client, &pool_state.token_0_vault)?;
    let token_1_balance = get_token_balance(rpc_client, &pool_state.token_1_vault)?;
    let user_n4_balance = get_token_balance(rpc_client, user_n4_account)?;
    let user_n5_balance = get_token_balance(rpc_client, user_n5_account)?;

    let price = calculate_price(token_0_balance, token_1_balance, n4_decimals, n5_decimals);
    let k = calculate_k(token_0_balance, token_1_balance);
    let calculated_lp_amount = calculate_lp_amount(token_0_balance, token_1_balance);
    let lp_supply_difference = calculated_lp_amount - pool_state.lp_supply as f64;

    println!("--- {} State ---", user_description);
    println!("N4 (SOL) balance: {:.6}", user_n4_balance as f64 / 10f64.powi(n4_decimals as i32));
    println!("N5 (USDC) balance: {:.6}", user_n5_balance as f64 / 10f64.powi(n5_decimals as i32));
    println!("Pool N4 (SOL) vault balance: {:.6}", token_0_balance as f64 / 10f64.powi(n4_decimals as i32));
    println!("Pool N5 (USDC) vault balance: {:.6}", token_1_balance as f64 / 10f64.powi(n5_decimals as i32));
    println!("Protocol fees (N4): {:.6}", pool_state.protocol_fees_token_0 as f64 / 10f64.powi(n4_decimals as i32));
    println!("Protocol fees (N5): {:.6}", pool_state.protocol_fees_token_1 as f64 / 10f64.powi(n5_decimals as i32));
    println!("Fund fees (N4): {:.6}", pool_state.fund_fees_token_0 as f64 / 10f64.powi(n4_decimals as i32));
    println!("Fund fees (N5): {:.6}", pool_state.fund_fees_token_1 as f64 / 10f64.powi(n5_decimals as i32));
    println!("Cumulative trade fees (N4): {:.6}", pool_state.cumulative_trade_fees_token_0 as f64 / 10f64.powi(n4_decimals as i32));
    println!("Cumulative trade fees (N5): {:.6}", pool_state.cumulative_trade_fees_token_1 as f64 / 10f64.powi(n5_decimals as i32));
    println!("Cumulative volume (N4): {:.6}", pool_state.cumulative_volume_token_0 as f64 / 10f64.powi(n4_decimals as i32));
    println!("Cumulative volume (N5): {:.6}", pool_state.cumulative_volume_token_1 as f64 / 10f64.powi(n5_decimals as i32));
    println!("LP supply: {:.6}", pool_state.lp_supply as f64 / 10f64.powi(9));
    println!("Price (USDC/SOL): {:.6}", price);
    println!("Constant product (k): {}", k);
    println!("Calculated LP amount: {:.6}", calculated_lp_amount);
    println!("LP supply difference: {:.6}", lp_supply_difference);
    println!("--------------------");

    Ok(())
}