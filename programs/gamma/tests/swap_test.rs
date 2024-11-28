use gamma::{
    curve::TradeDirection,
    states::{ObservationState, PoolState},
};
use solana_program_test::tokio;
use solana_sdk::{signature::Keypair, signer::Signer};
mod utils;

use utils::*;
// TODO: for future currently amm testing only works when `create_pool_fee` is 0. See how we can fix it to make it work for create_pool_fee > 0;

#[tokio::test]
async fn test_withdraw_deposit() {
    let user = Keypair::new();
    let admin = get_admin();
    let amm_index = 0;
    let mut test_env = TestEnv::new(vec![user.pubkey(), admin.pubkey()]).await;

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

    test_env.withdraw(&user, pool_id, amm_index, 1, 0, 0).await;
}

#[tokio::test]
async fn swap_base_input_dynamic_fee_test() {
    let user = Keypair::new();
    let admin = get_admin();
    let amm_index = 0;
    let mut test_env = TestEnv::new(vec![user.pubkey(), admin.pubkey()]).await;

    test_env
        .create_config(&admin, amm_index, 100, 20, 5, 0)
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

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            10000000,
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
            20000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;
    test_env.jump_seconds(100).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            3000000000000,
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
            40000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;

    test_env.jump_seconds(100).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            50000000,
            0,
            TradeDirection::OneForZero,
        )
        .await;

    test_env.jump_seconds(1).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            40000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;
    test_env.jump_seconds(100).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            50000000,
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
            50000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;
    test_env.jump_seconds(100).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            50000000,
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
            50000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;
    test_env.jump_seconds(100).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            50000000,
            0,
            TradeDirection::OneForZero,
        )
        .await;

    test_env.jump_seconds(30000).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            50000000,
            0,
            TradeDirection::OneForZero,
        )
        .await;

    test_env.jump_seconds(120000).await;

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            50000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;
}

#[tokio::test]
async fn swap_change_with_swaps() {
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
    dbg!(pool_state1.observation_key);
    let observation: ObservationState = test_env.fetch_account(pool_state1.observation_key).await;
    let pool_state1: PoolState = test_env.fetch_account(pool_id).await;

    let mut price_changes = vec![get_current_price_token_0_price(observation)];
    let mut pool_state_changes = vec![pool_state1];

    test_env.jump_seconds(100).await;

    ////////////////// Actual test start here /////////////
    ///////////////// The aim is to increase price0 and see change in dynamic fees.///////////////
    // We performed 3 swaps but the price is same as initial price.
    // For this trade we expect the dynamic fee to remain as base_fee
    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            300000000000000,
            0,
            TradeDirection::OneForZero,
        )
        .await;
    test_env.jump_seconds(100).await;
    // change of price before this trade

    let observation: ObservationState = test_env.fetch_account(pool_state1.observation_key).await;
    price_changes.push(get_current_price_token_0_price(observation));

    pool_state_changes.push(test_env.fetch_account(pool_id).await);

    let fee_charged_for_trade = pool_state_changes[pool_state_changes.len() - 1]
        .cumulative_trade_fees_token_1
        .saturating_sub(
            pool_state_changes[pool_state_changes.len() - 2].cumulative_trade_fees_token_1,
        );
    dbg!(fee_charged_for_trade);
    // 0.004333 => 0.4333% fee is charged on the trade amount 3000000000000
    assert_eq_with_copy!(fee_charged_for_trade, 300000000000);

    dbg!(&price_changes);
    // let percentage_change_in_price =
    //     ((price_changes[price_changes.len() - 1] - price_changes[price_changes.len() - 2]) * 1000)
    //         / price_changes[price_changes.len() - 2];
    // dbg!(percentage_change_in_price);
    // assert_eq!(percentage_change_in_price, 14948);

    // this trade will have even higher fees as price has increased
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
    // change of price before this trade

    let observation: ObservationState = test_env.fetch_account(pool_state1.observation_key).await;
    #[cfg(feature = "test-sbf")]
    {
        dbg!(observation);
    }
    price_changes.push(get_current_price_token_0_price(observation));

    pool_state_changes.push(test_env.fetch_account(pool_id).await);

    let fee_charged_for_trade = pool_state_changes[pool_state_changes.len() - 1]
        .cumulative_trade_fees_token_1
        .saturating_sub(
            pool_state_changes[pool_state_changes.len() - 2].cumulative_trade_fees_token_1,
        );
    dbg!(fee_charged_for_trade);
    // with increase in price by 14%
    // 0.014 => 0.9% fee is charged on the trade amount 3000000000000
    assert_eq_with_copy!(fee_charged_for_trade, 1);

    // this trade will have even higher fees as price has increased
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
    test_env.jump_seconds(16).await;
    // change of price before this trade

    let observation: ObservationState = test_env.fetch_account(pool_state1.observation_key).await;
    price_changes.push(get_current_price_token_0_price(observation));

    pool_state_changes.push(test_env.fetch_account(pool_id).await);

    let fee_charged_for_trade = pool_state_changes[pool_state_changes.len() - 1]
        .cumulative_trade_fees_token_1
        .saturating_sub(
            pool_state_changes[pool_state_changes.len() - 2].cumulative_trade_fees_token_1,
        );
    dbg!(fee_charged_for_trade);
    // 0.004333 => 0.4333% fee is charged on the trade amount 3000000000000
    assert_eq_with_copy!(fee_charged_for_trade, 78);

    // Now we do a ZeroForOne trade to decrease price and see change in dynamic fees.
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
    test_env.jump_seconds(16).await;
    let observation: ObservationState = test_env.fetch_account(pool_state1.observation_key).await;
    price_changes.push(get_current_price_token_0_price(observation));

    pool_state_changes.push(test_env.fetch_account(pool_id).await);

    let fee_charged_for_trade = pool_state_changes[pool_state_changes.len() - 1]
        .cumulative_trade_fees_token_0
        .saturating_sub(
            pool_state_changes[pool_state_changes.len() - 2].cumulative_trade_fees_token_0,
        );
    dbg!(fee_charged_for_trade);

    for _i in 0..100 {
        // Now we do a ZeroForOne trade to decrease price and see change in dynamic fees.
        test_env
            .swap_base_input(
                &user,
                pool_id,
                amm_index,
                1000000,
                0,
                TradeDirection::OneForZero,
            )
            .await;
        test_env.jump_seconds(16).await;
        let observation: ObservationState =
            test_env.fetch_account(pool_state1.observation_key).await;
        price_changes.push(get_current_price_token_0_price(observation));

        pool_state_changes.push(test_env.fetch_account(pool_id).await);

        let fee_charged_for_trade = pool_state_changes[pool_state_changes.len() - 1]
            .cumulative_trade_fees_token_1
            .saturating_sub(
                pool_state_changes[pool_state_changes.len() - 2].cumulative_trade_fees_token_1,
            );
        dbg!(fee_charged_for_trade);
    }

    for _i in 0..100 {
        // Now we do a ZeroForOne trade to decrease price and see change in dynamic fees.
        test_env
            .swap_base_input(
                &user,
                pool_id,
                amm_index,
                1000000,
                0,
                TradeDirection::ZeroForOne,
            )
            .await;
        test_env.jump_seconds(16).await;
        let observation: ObservationState =
            test_env.fetch_account(pool_state1.observation_key).await;
        price_changes.push(get_current_price_token_0_price(observation));

        pool_state_changes.push(test_env.fetch_account(pool_id).await);

        let fee_charged_for_trade = pool_state_changes[pool_state_changes.len() - 1]
            .cumulative_trade_fees_token_1
            .saturating_sub(
                pool_state_changes[pool_state_changes.len() - 2].cumulative_trade_fees_token_1,
            );
        dbg!(fee_charged_for_trade);
    }

    // let change_in_price =
    //     (price_changes[price_changes.len() - 1] - price_changes[price_changes.len() - 2])/;

    // let price_token0_1_trade = get_current_price_token_0_price(observation);
    // dbg!(price_token0_1_trade);
    // // assert!(price_token0_1_trade > price_token0_0th_trade);

    // // A fee of 6.4333% is charged on the trade amount 3000000000000
    // let percentage_increase_in_price =
    //     ((price_token0_1_trade - price_token0_0th_trade) * 1000) / price_token0_0th_trade;
    // dbg!(percentage_increase_in_price);

    // let pool_state3: PoolState = test_env.fetch_account(pool_id).await;
    // assert_eq_with_copy!(
    //     pool_state3.cumulative_trade_fees_token_0 - pool_state2.cumulative_trade_fees_token_0,
    //     0
    // );

    // 0.004333 => 0.4333% fee is charged on the trade amount 3000000000000
    // assert_eq_with_copy!(
    //     pool_state3.cumulative_trade_fees_token_1 - pool_state2.cumulative_trade_fees_token_1,
    //     300000000
    // );

    // we did a big trade increasing price by roughly 5.8%
    // assert_eq!(percentage_increase_in_price, 60);

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            300000,
            0,
            TradeDirection::OneForZero,
        )
        .await;

    test_env.jump_seconds(16).await;
    pool_state_changes.push(test_env.fetch_account(pool_id).await);

    let fee_charged_for_trade = pool_state_changes[pool_state_changes.len() - 1]
        .cumulative_trade_fees_token_1
        .saturating_sub(
            pool_state_changes[pool_state_changes.len() - 2].cumulative_trade_fees_token_1,
        );
    dbg!(fee_charged_for_trade);

    // let observation: ObservationState = test_env.fetch_account(pool_state2.observation_key).await;
    // let price_token0_2_trade = get_current_price_token_0_price(observation);
    // dbg!(price_token0_2_trade);

    // let percentage_increase_in_price =
    //     ((price_token0_2_trade - price_token0_1_trade) * 1000) / price_token0_1_trade;
    // dbg!(percentage_increase_in_price);

    // let pool_state4: PoolState = test_env.fetch_account(pool_id).await;
    // assert_eq_with_copy!(
    //     pool_state4.cumulative_trade_fees_token_0 - pool_state3.cumulative_trade_fees_token_0,
    //     0
    // );

    // last trade increased price by 60%
    // this causes a fee to increase from 0.064067 *100 = 6.4067%
    // assert_eq_with_copy!(
    //     pool_state4.cumulative_trade_fees_token_1 - pool_state3.cumulative_trade_fees_token_1,
    //     5
    // );

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         1000,
    //         0,
    //         TradeDirection::OneForZero,
    //     )
    //     .await;

    // test_env.jump_seconds(1).await;
    // let pool_state5: PoolState = test_env.fetch_account(pool_id).await;

    // This trade should also have same fee as last trade
    //
    // assert_eq_with_copy!(
    //     pool_state5.cumulative_trade_fees_token_1 - pool_state4.cumulative_trade_fees_token_1,
    //     5
    // );

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         40000000,
    //         0,
    //         TradeDirection::ZeroForOne,
    //     )
    //     .await;
    // test_env.jump_seconds(100).await;

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         50000000,
    //         0,
    //         TradeDirection::OneForZero,
    //     )
    //     .await;
    // test_env.jump_seconds(100).await;

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         50000000,
    //         0,
    //         TradeDirection::ZeroForOne,
    //     )
    //     .await;
    // test_env.jump_seconds(100).await;

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         50000000,
    //         0,
    //         TradeDirection::OneForZero,
    //     )
    //     .await;

    // test_env.jump_seconds(100).await;

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         50000000,
    //         0,
    //         TradeDirection::ZeroForOne,
    //     )
    //     .await;
    // test_env.jump_seconds(100).await;

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         50000000,
    //         0,
    //         TradeDirection::OneForZero,
    //     )
    //     .await;

    // test_env.jump_seconds(30000).await;

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         50000000,
    //         0,
    //         TradeDirection::OneForZero,
    //     )
    //     .await;

    // test_env.jump_seconds(120000).await;

    // test_env
    //     .swap_base_input(
    //         &user,
    //         pool_id,
    //         amm_index,
    //         50000000,
    //         0,
    //         TradeDirection::ZeroForOne,
    //     )
    //     .await;
}
