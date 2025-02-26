use std::time::{SystemTime, UNIX_EPOCH};

use anchor_spl::token::TokenAccount;
use gamma::{states::UserRewardInfo, REWARD_INFO_SEED, USER_REWARD_INFO_SEED};
use solana_program_test::tokio;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
mod utils;

use utils::*;

#[tokio::test]
async fn test_claim_rewards() {
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
    let end_time = timestamp_now + 3000;
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
    test_env
        .calculate_rewards(&user, pool_id, reward_info_key)
        .await;

    let (user_reward_info_key, _) = Pubkey::find_program_address(
        &[
            USER_REWARD_INFO_SEED.as_bytes(),
            reward_info_key.to_bytes().as_ref(),
            user.pubkey().to_bytes().as_ref(),
        ],
        &gamma::id(),
    );

    let user_reward_info: UserRewardInfo = test_env.fetch_account(user_reward_info_key).await;
    // Zero as it is before the start time of the reward
    assert_eq!(user_reward_info.total_rewards, 0);

    // Jump to end of rewardInfo
    test_env.jump_seconds(500000).await;
    test_env
        .calculate_rewards(&user, pool_id, reward_info_key)
        .await;
    let user_reward_info: UserRewardInfo = test_env.fetch_account(user_reward_info_key).await;
    assert_eq!(user_reward_info.total_rewards, 1000000000);

    test_env
        .claim_rewards(&user, pool_id, reward_info_key, reward_mint.pubkey())
        .await;
    let user_reward_info: UserRewardInfo = test_env.fetch_account(user_reward_info_key).await;
    assert_eq!(
        user_reward_info.total_claimed,
        user_reward_info.total_rewards
    );
    let users_mint_0_account = test_env
        .get_or_create_associated_token_account(user.pubkey(), reward_mint.pubkey(), &user)
        .await;
    let users_mint_0_account_info: TokenAccount =
        test_env.fetch_account(users_mint_0_account).await;
    assert_eq!(
        users_mint_0_account_info.amount,
        user_reward_info.total_claimed
    );
}
