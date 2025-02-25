use std::time::{SystemTime, UNIX_EPOCH};

use anchor_spl::token::TokenAccount;
use gamma::{states::RewardInfo, AUTH_SEED, REWARD_INFO_SEED, REWARD_VAULT_SEED};
use solana_program_test::tokio;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
mod utils;

use utils::*;

#[tokio::test]
async fn test_create_rewards() {
    let user = Keypair::new();
    let reward_provider = Keypair::new();
    let admin = get_admin();
    let amm_index = 0;
    let mut test_env = TestEnv::new(vec![
        user.pubkey(),
        admin.pubkey(),
        reward_provider.pubkey(),
    ])
    .await;
    test_env
        .create_config(&admin, amm_index, 100, 20, 5, 0)
        .await;

    let user_token_0_account = test_env
        .get_or_create_associated_token_account(user.pubkey(), test_env.token_0_mint, &user)
        .await;
    test_env
        .mint_base_tokens(user_token_0_account, 100000, test_env.token_0_mint)
        .await;

    let user_token_1_account = test_env
        .get_or_create_associated_token_account(user.pubkey(), test_env.token_1_mint, &user)
        .await;
    test_env
        .mint_base_tokens(user_token_1_account, 100000, test_env.token_1_mint)
        .await;

    let pool_id = test_env
        .initialize_pool(
            &user,
            amm_index,
            1000,
            2000,
            0,
            gamma::create_pool_fee_reveiver::id(),
        )
        .await;

    test_env
        .deposit(&user, pool_id, amm_index, 1, 999999, 99999)
        .await;

    let reward_mint = Keypair::new();
    test_env
        .create_token_mint(&reward_mint, &test_env.mint_authority.pubkey(), 9)
        .await;

    let reward_provider_token_account = test_env
        .get_or_create_associated_token_account(
            reward_provider.pubkey(),
            reward_mint.pubkey(),
            &reward_provider,
        )
        .await;
    let reward_amount = 1000000000;

    test_env
        .mint_base_tokens(
            reward_provider_token_account,
            reward_amount,
            reward_mint.pubkey(),
        )
        .await;

    let timestamp_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let start_time = timestamp_now + 10;
    let end_time = timestamp_now + 200;
    test_env
        .create_rewards(
            &reward_provider,
            pool_id,
            start_time,
            end_time,
            reward_mint.pubkey(),
            reward_amount,
        )
        .await;

    let (reward_info_key, _) = Pubkey::find_program_address(
        &[
            REWARD_INFO_SEED.as_bytes(),
            pool_id.to_bytes().as_ref(),
            &start_time.to_le_bytes(),
            reward_mint.pubkey().to_bytes().as_ref(),
        ],
        &gamma::id(),
    );

    let (reward_vault_key, _) = Pubkey::find_program_address(
        &[
            REWARD_VAULT_SEED.as_bytes(),
            reward_info_key.to_bytes().as_ref(),
        ],
        &gamma::id(),
    );

    let reward_info: RewardInfo = test_env.fetch_account(reward_info_key).await;
    assert_eq!(reward_info.total_to_disburse, reward_amount);
    assert_eq!(reward_info.start_at, start_time);
    assert_eq!(reward_info.end_rewards_at, end_time);

    let reward_vault: TokenAccount = test_env.fetch_account(reward_vault_key).await;
    assert_eq!(reward_vault.amount, reward_amount);
    assert_eq!(reward_vault.mint, reward_mint.pubkey());

    let (authority, _) = Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &gamma::id());
    assert_eq!(reward_vault.owner, authority);
}
