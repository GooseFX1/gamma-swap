use std::collections::HashMap;

use anchor_spl::token::TokenAccount;
use gamma::{curve::TradeDirection, states::PoolState};
use solana_program_test::tokio;
use solana_sdk::{clock::Clock, signature::Keypair, signer::Signer};
mod utils;
use jupiter_amm_interface::{AccountMap, Amm, AmmContext, ClockRef, KeyedAccount, SwapMode};
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
    test_env.jump_seconds(100).await;
    let pool_state_info = test_env.get_account_info(pool_id).await.unwrap().unwrap();
    let keyed_account = KeyedAccount {
        key: pool_id,
        account: pool_state_info,
        params: None,
    };
    let clock: Clock = test_env
        .program_test_context
        .banks_client
        .get_sysvar()
        .await
        .unwrap();
    let clock_ref = ClockRef::from(clock);

    let amm_context: AmmContext = AmmContext { clock_ref };
    let mut jupiter_quote_result =
        jupiter::Gamma::from_keyed_account(&keyed_account, &amm_context).unwrap();
    let mut pool_state_before: PoolState = test_env.fetch_account(pool_id).await;

    let user_token_0_pk = test_env
        .get_or_create_associated_token_account(user.pubkey(), test_env.token_0_mint, &user)
        .await;
    let user_token_1_pk = test_env
        .get_or_create_associated_token_account(user.pubkey(), test_env.token_1_mint, &user)
        .await;

    let hasher = ahash::RandomState::new();
    let mut account_map: AccountMap = HashMap::with_hasher(hasher);

    ////////////////// Actual test start here /////////////
    for _ in 0..100 {
        let swap_amount = 100000000000;
        let pool_state_info = test_env.get_account_info(pool_id).await.unwrap().unwrap();
        let token_0_mint = test_env
            .get_account_info(pool_state0.token_0_mint)
            .await
            .unwrap()
            .unwrap();
        let token_1_mint = test_env
            .get_account_info(pool_state0.token_1_mint)
            .await
            .unwrap()
            .unwrap();
        let amm_config = test_env
            .get_account_info(pool_state0.amm_config)
            .await
            .unwrap()
            .unwrap();
        let observation_state_info = test_env
            .get_account_info(pool_state0.observation_key)
            .await
            .unwrap()
            .unwrap();

        let token_0_vault = test_env
            .get_account_info(pool_state0.token_0_vault)
            .await
            .unwrap()
            .unwrap();
        let token_1_vault = test_env
            .get_account_info(pool_state0.token_1_vault)
            .await
            .unwrap()
            .unwrap();
        account_map.insert(pool_state0.token_0_mint, token_0_mint);
        account_map.insert(pool_state0.token_1_mint, token_1_mint);
        account_map.insert(pool_state0.amm_config, amm_config);
        account_map.insert(pool_id, pool_state_info);
        account_map.insert(pool_state0.observation_key, observation_state_info);
        account_map.insert(pool_state0.token_0_vault, token_0_vault);
        account_map.insert(pool_state0.token_1_vault, token_1_vault);

        let clock: Clock = test_env
            .program_test_context
            .banks_client
            .get_sysvar()
            .await
            .unwrap();
        amm_context.clock_ref.update(clock);

        jupiter_quote_result.update(&account_map).unwrap();
        let quote = jupiter_quote_result
            .quote(&jupiter_amm_interface::QuoteParams {
                amount: swap_amount,
                input_mint: test_env.token_0_mint,
                output_mint: test_env.token_1_mint,
                swap_mode: SwapMode::ExactIn,
            })
            .unwrap();

        let user_token_0_account_before: TokenAccount =
            test_env.fetch_account(user_token_0_pk).await;
        let user_token_1_account_before: TokenAccount =
            test_env.fetch_account(user_token_1_pk).await;

        // perform actual swap
        test_env
            .swap_base_input(
                &user,
                pool_id,
                amm_index,
                swap_amount,
                0,
                TradeDirection::ZeroForOne,
            )
            .await;

        let user_token_0_account_after: TokenAccount =
            test_env.fetch_account(user_token_0_pk).await;
        let user_token_1_account_after: TokenAccount =
            test_env.fetch_account(user_token_1_pk).await;

        let change_in_token_0 = user_token_0_account_after
            .amount
            .abs_diff(user_token_0_account_before.amount);
        let change_in_token_1 = user_token_1_account_after
            .amount
            .abs_diff(user_token_1_account_before.amount);

        assert_eq!(swap_amount, change_in_token_0);
        assert_eq!(quote.in_amount, change_in_token_0);
        assert_eq!(quote.out_amount, change_in_token_1);

        let pool_state_after: PoolState = test_env.fetch_account(pool_id).await;
        let fees_charged = pool_state_after.cumulative_trade_fees_token_0
            - pool_state_before.cumulative_trade_fees_token_0;
        assert_eq!(fees_charged as u64, quote.fee_amount);
        pool_state_before = pool_state_after;
        test_env.jump_seconds(16).await;
    }

    for _ in 0..100 {
        let swap_amount = 100000;
        let pool_state_info = test_env.get_account_info(pool_id).await.unwrap().unwrap();
        let token_0_mint = test_env
            .get_account_info(pool_state0.token_0_mint)
            .await
            .unwrap()
            .unwrap();
        let token_1_mint = test_env
            .get_account_info(pool_state0.token_1_mint)
            .await
            .unwrap()
            .unwrap();
        let amm_config = test_env
            .get_account_info(pool_state0.amm_config)
            .await
            .unwrap()
            .unwrap();
        let observation_state_info = test_env
            .get_account_info(pool_state0.observation_key)
            .await
            .unwrap()
            .unwrap();

        let token_0_vault = test_env
            .get_account_info(pool_state0.token_0_vault)
            .await
            .unwrap()
            .unwrap();
        let token_1_vault = test_env
            .get_account_info(pool_state0.token_1_vault)
            .await
            .unwrap()
            .unwrap();
        account_map.insert(pool_state0.token_0_mint, token_0_mint);
        account_map.insert(pool_state0.token_1_mint, token_1_mint);
        account_map.insert(pool_state0.amm_config, amm_config);
        account_map.insert(pool_id, pool_state_info);
        account_map.insert(pool_state0.observation_key, observation_state_info);
        account_map.insert(pool_state0.token_0_vault, token_0_vault);
        account_map.insert(pool_state0.token_1_vault, token_1_vault);

        jupiter_quote_result.update(&account_map).unwrap();

        let clock: Clock = test_env
            .program_test_context
            .banks_client
            .get_sysvar()
            .await
            .unwrap();
        amm_context.clock_ref.update(clock);
        let quote = jupiter_quote_result
            .quote(&jupiter_amm_interface::QuoteParams {
                amount: swap_amount,
                input_mint: test_env.token_1_mint,
                output_mint: test_env.token_0_mint,
                swap_mode: SwapMode::ExactIn,
            })
            .unwrap();

        let user_token_0_account_before: TokenAccount =
            test_env.fetch_account(user_token_0_pk).await;
        let user_token_1_account_before: TokenAccount =
            test_env.fetch_account(user_token_1_pk).await;

        // perform actual swap
        test_env
            .swap_base_input(
                &user,
                pool_id,
                amm_index,
                swap_amount,
                0,
                TradeDirection::OneForZero,
            )
            .await;

        let user_token_0_account_after: TokenAccount =
            test_env.fetch_account(user_token_0_pk).await;
        let user_token_1_account_after: TokenAccount =
            test_env.fetch_account(user_token_1_pk).await;

        let change_in_token_0 = user_token_0_account_after
            .amount
            .abs_diff(user_token_0_account_before.amount);
        let change_in_token_1 = user_token_1_account_after
            .amount
            .abs_diff(user_token_1_account_before.amount);

        assert_eq!(swap_amount, change_in_token_1);
        assert_eq!(quote.in_amount, change_in_token_1);
        assert_eq!(quote.out_amount, change_in_token_0);

        let pool_state_after: PoolState = test_env.fetch_account(pool_id).await;
        let fees_charged = pool_state_after.cumulative_trade_fees_token_1
            - pool_state_before.cumulative_trade_fees_token_1;
        assert_eq!(fees_charged as u64, quote.fee_amount);
        pool_state_before = pool_state_after;

        test_env.jump_seconds(16).await;
    }
}
