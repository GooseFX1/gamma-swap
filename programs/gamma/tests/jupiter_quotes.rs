use gamma::{
    curve::TradeDirection,
    states::{ObservationState, PoolState},
};
use solana_program_test::tokio;
use solana_sdk::{signature::Keypair, signer::Signer};
mod utils;
use utils::jupiter;

use utils::*;

#[tokio::test]
async fn jupiter_quotes() {
    // Setup
    let user = Keypair::new();
    let admin = get_admin();
    let amm_index = 0;
    let mut test_env = TestEnv::new(vec![user.pubkey(), admin.pubkey()]).await;

    test_env
        .create_config(&admin, amm_index, 1000, 20, 5, 0)
        .await;

    let user_token_0_account = test_env
        .get_or_create_associated_token_account(user.pubkey(), test_env.token_0_mint, &user)
        .await;
    test_env
        .mint_base_tokens(user_token_0_account, 100000000000000, test_env.token_0_mint)
        .await;

    let user_token_1_account = test_env
        .get_or_create_associated_token_account(user.pubkey(), test_env.token_1_mint, &user)
        .await;

    test_env
        .mint_base_tokens(
            user_token_1_account,
            1000000000000000,
            test_env.token_1_mint,
        )
        .await;

    let pool_id = test_env
        .initialize_pool(
            &user,
            amm_index,
            20000000000000,
            10000000000000,
            0,
            gamma::create_pool_fee_reveiver::id(),
        )
        .await;
    // we jump 100 seconds in time to make sure current blockTime is more than pool.open_time
    test_env.jump_seconds(100).await;

    let pool_state0: PoolState = test_env.fetch_account(pool_id).await;
    assert_eq_with_copy!(pool_state0.cumulative_trade_fees_token_0, 0);
    assert_eq_with_copy!(pool_state0.cumulative_trade_fees_token_1, 0);

    // dummy swaps to set initial observation of price.

    let pool_state1: PoolState = test_env.fetch_account(pool_id).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            1000,
            0,
            TradeDirection::OneForZero,
        )
        .await;

    test_env.jump_seconds(100).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            1000,
            0,
            TradeDirection::OneForZero,
        )
        .await;

    test_env.jump_seconds(100).await;
    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            2000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;

    ///// We make at 3 swaps to set initial observation of price. /////
    let observation: ObservationState = test_env.fetch_account(pool_state1.observation_key).await;
    let pool_state1: PoolState = test_env.fetch_account(pool_id).await;

    let mut price_changes = vec![get_current_price_token_0_price(observation)];
    let mut pool_state_changes = vec![pool_state1];

    test_env.jump_seconds(100).await;

    ////////////////// Actual test start here /////////////
}
