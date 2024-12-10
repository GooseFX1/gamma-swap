use std::u64;

use gamma::curve::TradeDirection;
use gamma::states::PartnerType;
use gamma::states::PoolState;
use solana_program_test::tokio;
use solana_sdk::{signature::Keypair, signer::Signer};
mod utils;

use utils::*;

#[tokio::test]
async fn should_track_cumulative_rates_correctly() {
    // Setup
    let user = Keypair::new();
    let lp_depositor_asset_dash = Keypair::new();

    let admin = get_admin();
    let amm_index = 0;
    let mut test_env = TestEnv::new(vec![
        user.pubkey(),
        lp_depositor_asset_dash.pubkey(),
        admin.pubkey(),
    ])
    .await;

    test_env
        .create_config(&admin, amm_index, 3000, 2000, 50, 0)
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

    let lp_depositor_asset_dash_token0 = test_env
        .get_or_create_associated_token_account(
            lp_depositor_asset_dash.pubkey(),
            test_env.token_0_mint,
            &lp_depositor_asset_dash,
        )
        .await;
    test_env
        .mint_base_tokens(
            lp_depositor_asset_dash_token0,
            100000000000000,
            test_env.token_0_mint,
        )
        .await;

    let lp_depositor_asset_dash_token1 = test_env
        .get_or_create_associated_token_account(
            lp_depositor_asset_dash.pubkey(),
            test_env.token_1_mint,
            &lp_depositor_asset_dash,
        )
        .await;

    test_env
        .mint_base_tokens(
            lp_depositor_asset_dash_token1,
            1000000000000000,
            test_env.token_1_mint,
        )
        .await;

    let pool_id = test_env
        .initialize_pool(
            &user,
            amm_index,
            200000000,
            100000000,
            0,
            gamma::create_pool_fee_reveiver::id(),
        )
        .await;
    // we jump 100 seconds in time to make sure current blockTime is more than pool.open_time
    test_env.jump_seconds(100).await;

    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    dbg!(1, pool_state.token_0_vault_amount, pool_state.token_1_vault_amount);

    assert_eq_with_copy!(pool_state.cumulative_trade_fees_token_0, 0);
    assert_eq_with_copy!(pool_state.cumulative_trade_fees_token_1, 0);

    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_0,
        0
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_1,
        0
    );

    assert_eq_with_copy!(pool_state.partners[0].lp_token_linked_with_partner, 0);
    assert_eq_with_copy!(
        PartnerType::new(pool_state.partners[0].partner_id),
        PartnerType::AssetDash
    );
    assert_eq_with_copy!(pool_state.partners[0].partner_id, 0);

    test_env
        .init_user_pool_liquidity_with_partner(
            &lp_depositor_asset_dash,
            pool_id,
            Some("AssetDash".to_string()),
        )
        .await;

    let lp_deposit_amount = 200000000;
    test_env
        .deposit(
            &lp_depositor_asset_dash,
            pool_id,
            amm_index,
            lp_deposit_amount,
            u64::MAX,
            u64::MAX,
        )
        .await;

    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    dbg!(2, pool_state.token_0_vault_amount, pool_state.token_1_vault_amount);


    assert_eq_with_copy!(
        pool_state.partners[0].lp_token_linked_with_partner,
        lp_deposit_amount
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_0,
        0
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_1,
        0
    );

    let withdraw_amount = 100000000;
    test_env
        .withdraw(
            &lp_depositor_asset_dash,
            pool_id,
            amm_index,
            withdraw_amount,
            0,
            0,
        )
        .await;

    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    dbg!(3, pool_state.token_0_vault_amount, pool_state.token_1_vault_amount);

    assert_eq_with_copy!(
        pool_state.partners[0].lp_token_linked_with_partner,
        lp_deposit_amount - withdraw_amount
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_0,
        0
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_1,
        0
    );

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            1000000000,
            0,
            TradeDirection::OneForZero,
        )
        .await;

    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    dbg!(4, pool_state.token_0_vault_amount, pool_state.token_1_vault_amount);

    assert_eq_with_copy!(
        pool_state.partners[0].lp_token_linked_with_partner,
        lp_deposit_amount - withdraw_amount
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_0,
        0
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_1,
        (pool_state.partners[0].lp_token_linked_with_partner * pool_state.protocol_fees_token_1)
            / pool_state.lp_supply
    );

    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            1000000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;

    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    dbg!(5, pool_state.token_0_vault_amount, pool_state.token_1_vault_amount);

    assert_eq_with_copy!(
        pool_state.partners[0].lp_token_linked_with_partner,
        lp_deposit_amount - withdraw_amount
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_0,
        (pool_state.partners[0].lp_token_linked_with_partner * pool_state.protocol_fees_token_0)
            / pool_state.lp_supply
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_1,
        (pool_state.partners[0].lp_token_linked_with_partner * pool_state.protocol_fees_token_1)
            / pool_state.lp_supply
    );

    // swap base output

    test_env
        .swap_base_output(
            &user,
            pool_id,
            amm_index,
            1000000000,
            u64::MAX,
            TradeDirection::OneForZero,
        )
        .await;

    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    dbg!(6, pool_state.token_0_vault_amount, pool_state.token_1_vault_amount);

    assert_eq_with_copy!(
        pool_state.partners[0].lp_token_linked_with_partner,
        lp_deposit_amount - withdraw_amount
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_0,
        (pool_state.partners[0].lp_token_linked_with_partner * pool_state.protocol_fees_token_0)
            / pool_state.lp_supply
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_1,
        (pool_state.partners[0].lp_token_linked_with_partner * pool_state.protocol_fees_token_1)
            / pool_state.lp_supply
    );

    test_env
        .swap_base_output(
            &user,
            pool_id,
            amm_index,
            1000000000,
            u64::MAX,
            TradeDirection::ZeroForOne,
        )
        .await;

    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    dbg!(7, pool_state.token_0_vault_amount, pool_state.token_1_vault_amount);

    assert_eq_with_copy!(
        pool_state.partners[0].lp_token_linked_with_partner,
        lp_deposit_amount - withdraw_amount
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_0,
        (pool_state.partners[0].lp_token_linked_with_partner * pool_state.protocol_fees_token_0)
            / pool_state.lp_supply
    );
    assert_eq_with_copy!(
        pool_state.partners[0].cumulative_fee_total_times_tvl_share_token_1,
        (pool_state.partners[0].lp_token_linked_with_partner * pool_state.protocol_fees_token_1)
            / pool_state.lp_supply
    );
}
