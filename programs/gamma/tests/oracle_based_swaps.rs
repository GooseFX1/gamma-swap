use anchor_spl::token::TokenAccount;
use gamma::{
    curve::{SwapResult, TradeDirection},
    states::{AmmConfig, ObservationState, PoolState},
};
use solana_program_test::tokio;
use solana_sdk::{clock::Clock, pubkey::Pubkey, signature::Keypair, signer::Signer};
mod utils;
use utils::*;

async fn test_env_setup(user: &Keypair, admin: &Keypair) -> (TestEnv, Pubkey) {
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
    (test_env, pool_id)
}

async fn get_quote(
    test_env: &mut TestEnv,
    pool_id: Pubkey,
    amount_in: u64,
    trade_direction: TradeDirection,
) -> SwapResult {
    let pool_config = test_env.fetch_account::<PoolState>(pool_id).await;
    let amm_config = test_env
        .fetch_account::<AmmConfig>(pool_config.amm_config)
        .await;
    let observation_state = test_env
        .fetch_account::<ObservationState>(pool_config.observation_key)
        .await;
    let clock: Clock = test_env
        .program_test_context
        .banks_client
        .get_sysvar()
        .await
        .unwrap();

    gamma::curve::CurveCalculator::swap_base_input(
        amount_in.into(),
        if trade_direction == TradeDirection::OneForZero {
            pool_config.token_1_vault_amount.into()
        } else {
            pool_config.token_0_vault_amount.into()
        },
        if trade_direction == TradeDirection::OneForZero {
            pool_config.token_0_vault_amount.into()
        } else {
            pool_config.token_1_vault_amount.into()
        },
        &amm_config,
        &pool_config,
        clock.unix_timestamp as u64,
        &observation_state,
        false,
    )
    .unwrap()
}

async fn get_user_token_amounts(
    test_env: &mut TestEnv,
    pool_id: Pubkey,
    user: &Keypair,
) -> (u64, u64) {
    let pool_config = test_env.fetch_account::<PoolState>(pool_id).await;
    let user_token_0_account = test_env
        .get_or_create_associated_token_account(user.pubkey(), pool_config.token_0_mint, user)
        .await;
    let user_token_1_account = test_env
        .get_or_create_associated_token_account(user.pubkey(), pool_config.token_1_mint, user)
        .await;
    (
        test_env
            .fetch_account::<TokenAccount>(user_token_0_account)
            .await
            .amount,
        test_env
            .fetch_account::<TokenAccount>(user_token_1_account)
            .await
            .amount,
    )
}

pub fn amount_out(
    before_amount: (u64, u64),
    after_amount: (u64, u64),
    trade_direction: TradeDirection,
) -> u128 {
    if trade_direction == TradeDirection::OneForZero {
        (after_amount.0 as u128 - before_amount.0 as u128).into()
    } else {
        (after_amount.1 as u128 - before_amount.1 as u128).into()
    }
}

mod oracle_based_swaps {
    use super::*;
    use gamma::{curve::D9, fees::FEE_RATE_DENOMINATOR_VALUE};

    mod zero_for_one {
        use super::*;

        #[tokio::test]
        async fn should_swap_with_old_calculator_if_oracle_price_is_not_updated() {
            let user = Keypair::new();
            let admin = get_admin();
            let amm_index = 0;

            let (mut test_env, pool_id) = test_env_setup(&user, &admin).await;

            // Update config of pool.
            // Acceptable price difference Set to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    6,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Allow 1% of TVL to be swappable at oracle price
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    7,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set min trade rate at oracle price to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    8,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set price premium for swap at oracle price to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    9,
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            let trade_direction = TradeDirection::ZeroForOne;
            let amount_in = 10000000;
            let quote = get_quote(&mut test_env, pool_id, amount_in, trade_direction).await;
            let before_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            test_env
                .oracle_based_swap_base_input(
                    &user,
                    pool_id,
                    amm_index,
                    amount_in,
                    0,
                    trade_direction,
                )
                .await;
            let after_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            let amount_out = amount_out(before_amounts, after_amounts, trade_direction);
            // We expect the amount out to exactly what is quoted with old calculator
            assert_eq!(quote.destination_amount_swapped, amount_out);
        }

        #[tokio::test]
        async fn should_swap_with_new_calculator_if_oracle_price_is_updated() {
            let user = Keypair::new();
            let admin = get_admin();
            let amm_index = 0;

            let (mut test_env, pool_id) = test_env_setup(&user, &admin).await;

            // Update config of pool.
            // Acceptable price difference Set to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    6,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Allow 1% of TVL to be swappable at oracle price
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    7,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set min trade rate at oracle price to 0.1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    8,
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            // Set price premium for swap at oracle price to 0.01%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    9, // 1_000_000
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            test_env
                // Set it same as the current pool price
                .oracle_price_update(
                    &admin,
                    pool_id,
                    amm_index,
                    20000000000000 * D9 / 10000000000000,
                )
                .await;

            let trade_direction = TradeDirection::ZeroForOne;
            let amount_in = 10000000;
            let quote = get_quote(&mut test_env, pool_id, amount_in, trade_direction).await;
            let before_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            test_env
                .oracle_based_swap_base_input(
                    &user,
                    pool_id,
                    amm_index,
                    amount_in,
                    0,
                    trade_direction,
                )
                .await;
            let after_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            let amount_out = amount_out(before_amounts, after_amounts, trade_direction);
            // We expect the amount out to be more than what is quoted with old calculator
            dbg!(quote.destination_amount_swapped);
            dbg!(amount_out);
            assert!(quote.destination_amount_swapped < amount_out);
        }

        #[tokio::test]
        async fn should_use_old_calculator_if_amount_in_is_large() {
            let user = Keypair::new();
            let admin = get_admin();
            let amm_index = 0;

            let (mut test_env, pool_id) = test_env_setup(&user, &admin).await;

            // Update config of pool.
            // Acceptable price difference Set to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    6,
                    FEE_RATE_DENOMINATOR_VALUE / 1000,
                )
                .await;

            // Allow 1% of TVL to be swappable at oracle price
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    7,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set min trade rate at oracle price to 0.1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    8,
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            // Set price premium for swap at oracle price to 0.01%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    9, // 1_000_000
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            test_env
                // Set it same as the current pool price
                .oracle_price_update(
                    &admin,
                    pool_id,
                    amm_index,
                    20000000000000 * D9 / 10000000000000,
                )
                .await;

            let trade_direction = TradeDirection::ZeroForOne;
            let amount_in = 100000000000;
            let quote = get_quote(&mut test_env, pool_id, amount_in, trade_direction).await;
            let before_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            test_env
                .oracle_based_swap_base_input(
                    &user,
                    pool_id,
                    amm_index,
                    amount_in,
                    0,
                    trade_direction,
                )
                .await;
            let after_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            let amount_out = amount_out(before_amounts, after_amounts, trade_direction);
            // We expect the amount out to be more than what is quoted with old calculator
            dbg!(quote.destination_amount_swapped);
            dbg!(amount_out);
            assert!(quote.destination_amount_swapped < amount_out);
            // We expect some amount to be traded with the old calculator
            // With logs I was able to verify it changes, but for assets, this should be good.
            assert!(amount_out < (20000000000000_u128 * amount_in as u128) / 10000000000000);
        }
    }

    mod one_for_zero {
        use super::*;

        #[tokio::test]
        async fn should_swap_with_old_calculator_if_oracle_price_is_not_updated() {
            let user = Keypair::new();
            let admin = get_admin();
            let amm_index = 0;

            let (mut test_env, pool_id) = test_env_setup(&user, &admin).await;

            // Update config of pool.
            // Acceptable price difference Set to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    6,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Allow 1% of TVL to be swappable at oracle price
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    7,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set min trade rate at oracle price to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    8,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set price premium for swap at oracle price to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    9,
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            let trade_direction = TradeDirection::OneForZero;
            let amount_in = 10000000;
            let quote = get_quote(&mut test_env, pool_id, amount_in, trade_direction).await;
            let before_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            test_env
                .oracle_based_swap_base_input(
                    &user,
                    pool_id,
                    amm_index,
                    amount_in,
                    0,
                    trade_direction,
                )
                .await;
            let after_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            let amount_out = amount_out(before_amounts, after_amounts, trade_direction);
            // We expect the amount out to exactly what is quoted with old calculator
            assert_eq!(quote.destination_amount_swapped, amount_out);
        }

        #[tokio::test]
        async fn should_swap_with_new_calculator_if_oracle_price_is_updated() {
            let user = Keypair::new();
            let admin = get_admin();
            let amm_index = 0;

            let (mut test_env, pool_id) = test_env_setup(&user, &admin).await;

            // Update config of pool.
            // Acceptable price difference Set to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    6,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Allow 1% of TVL to be swappable at oracle price
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    7,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set min trade rate at oracle price to 0.1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    8,
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            // Set price premium for swap at oracle price to 0.01%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    9, // 1_000_000
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            test_env
                // Set it same as the current pool price
                .oracle_price_update(
                    &admin,
                    pool_id,
                    amm_index,
                    20000000000000 * D9 / 10000000000000,
                )
                .await;

            let trade_direction = TradeDirection::OneForZero;
            let amount_in = 10000000;
            let quote = get_quote(&mut test_env, pool_id, amount_in, trade_direction).await;
            let before_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            test_env
                .oracle_based_swap_base_input(
                    &user,
                    pool_id,
                    amm_index,
                    amount_in,
                    0,
                    trade_direction,
                )
                .await;
            let after_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            let amount_out = amount_out(before_amounts, after_amounts, trade_direction);
            // We expect the amount out to be more than what is quoted with old calculator
            dbg!(quote.destination_amount_swapped);
            dbg!(amount_out);
            assert!(quote.destination_amount_swapped < amount_out);
        }

        #[tokio::test]
        async fn should_use_old_calculator_if_amount_in_is_large() {
            let user = Keypair::new();
            let admin = get_admin();
            let amm_index = 0;

            let (mut test_env, pool_id) = test_env_setup(&user, &admin).await;

            // Update config of pool.
            // Acceptable price difference Set to 1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    6,
                    FEE_RATE_DENOMINATOR_VALUE / 1000,
                )
                .await;

            // Allow 1% of TVL to be swappable at oracle price
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    7,
                    FEE_RATE_DENOMINATOR_VALUE / 100,
                )
                .await;

            // Set min trade rate at oracle price to 0.1%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    8,
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            // Set price premium for swap at oracle price to 0.01%
            test_env
                .update_pool(
                    &admin,
                    pool_id,
                    amm_index,
                    9, // 1_000_000
                    FEE_RATE_DENOMINATOR_VALUE / 10000,
                )
                .await;

            test_env
                // Set it same as the current pool price
                .oracle_price_update(
                    &admin,
                    pool_id,
                    amm_index,
                    20000000000000 * D9 / 10000000000000,
                )
                .await;

            let trade_direction = TradeDirection::OneForZero;
            let amount_in = 100000000000;
            let quote = get_quote(&mut test_env, pool_id, amount_in, trade_direction).await;
            let before_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            test_env
                .oracle_based_swap_base_input(
                    &user,
                    pool_id,
                    amm_index,
                    amount_in,
                    0,
                    trade_direction,
                )
                .await;
            let after_amounts = get_user_token_amounts(&mut test_env, pool_id, &user).await;
            let amount_out = amount_out(before_amounts, after_amounts, trade_direction);
            // We expect the amount out to be more than what is quoted with old calculator
            dbg!(quote.destination_amount_swapped);
            dbg!(amount_out);
            assert!(quote.destination_amount_swapped < amount_out);
            // We expect some amount to be traded with the old calculator
            // With logs I was able to verify it changes, but for assets, this should be good.
            assert!(amount_out < (20000000000000_u128 * amount_in as u128) / 10000000000000);
        }
    }
}
