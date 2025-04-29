use std::u64;

use gamma::curve::TradeDirection;
use gamma::fees::FEE_RATE_DENOMINATOR_VALUE;
use gamma::states::{Partner, PoolPartnerInfos, PoolState, UserPoolLiquidity};
use solana_program_test::tokio;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
mod utils;

use utils::*;

#[tokio::test]
async fn should_track_cumulative_rates_correctly() {
    // Setup
    let user = Keypair::new();
    let depositor1 = Keypair::new();
    let depositor2 = Keypair::new();
    let partner_1_authority = Keypair::new();
    let partner_2_authority = Keypair::new();
    let partner_share_rate = 100_000; // 10%

    let admin = get_admin();
    let amm_index = 0;
    let mut test_env = TestEnv::new(vec![
        user.pubkey(),
        depositor1.pubkey(),
        depositor2.pubkey(),
        admin.pubkey(),
        partner_1_authority.pubkey(),
        partner_2_authority.pubkey(),
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

    let partner_1_token_0_account = test_env
        .get_or_create_associated_token_account(
            partner_1_authority.pubkey(),
            test_env.token_0_mint,
            &partner_1_authority,
        )
        .await;
    let partner_1_token_1_account = test_env
        .get_or_create_associated_token_account(
            partner_1_authority.pubkey(),
            test_env.token_1_mint,
            &partner_1_authority,
        )
        .await;

    let partner_2_token_0_account = test_env
        .get_or_create_associated_token_account(
            partner_2_authority.pubkey(),
            test_env.token_0_mint,
            &partner_2_authority,
        )
        .await;
    let partner_2_token_1_account = test_env
        .get_or_create_associated_token_account(
            partner_2_authority.pubkey(),
            test_env.token_1_mint,
            &partner_2_authority,
        )
        .await;

    test_env
        .mint_base_tokens(
            user_token_1_account,
            1000000000000000,
            test_env.token_1_mint,
        )
        .await;

    // depositor 1 setup:
    let depositor1_token0 = test_env
        .get_or_create_associated_token_account(
            depositor1.pubkey(),
            test_env.token_0_mint,
            &depositor1,
        )
        .await;
    test_env
        .mint_base_tokens(depositor1_token0, 100000000000000, test_env.token_0_mint)
        .await;

    let depositor1_token1 = test_env
        .get_or_create_associated_token_account(
            depositor1.pubkey(),
            test_env.token_1_mint,
            &depositor1,
        )
        .await;
    test_env
        .mint_base_tokens(depositor1_token1, 1000000000000000, test_env.token_1_mint)
        .await;

    // lp depositor 2 setup:
    let depositor2_token0 = test_env
        .get_or_create_associated_token_account(
            depositor2.pubkey(),
            test_env.token_0_mint,
            &depositor2,
        )
        .await;
    test_env
        .mint_base_tokens(depositor2_token0, 100000000000000, test_env.token_0_mint)
        .await;

    let depositor2_token1 = test_env
        .get_or_create_associated_token_account(
            depositor2.pubkey(),
            test_env.token_1_mint,
            &depositor2,
        )
        .await;
    test_env
        .mint_base_tokens(depositor2_token1, 1000000000000000, test_env.token_1_mint)
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
    let initial_lp_supply = pool_state.lp_supply;
    dbg!(
        1,
        pool_state.token_0_vault_amount,
        pool_state.token_1_vault_amount
    );

    assert_eq_with_copy!(pool_state.cumulative_trade_fees_token_0, 0);
    assert_eq_with_copy!(pool_state.cumulative_trade_fees_token_1, 0);
    assert_eq_with_copy!(pool_state.protocol_fees_token_0, 0);
    assert_eq_with_copy!(pool_state.protocol_fees_token_1, 0);
    assert_eq_with_copy!(pool_state.partner_protocol_fees_token_0, 0);
    assert_eq_with_copy!(pool_state.partner_protocol_fees_token_1, 0);
    assert_eq_with_copy!(pool_state.partner_share_rate, 0);

    // update partner
    test_env
        .update_pool(&admin, pool_id, amm_index, 11, partner_share_rate)
        .await;
    let pool_state: PoolState = test_env.fetch_account(pool_id).await;
    assert_eq_with_copy!(pool_state.partner_share_rate, partner_share_rate);

    let pool_partners_key = derive_pool_partners_pda(pool_id).0;
    let partner1 = test_env
        .initialize_partner(
            &partner_1_authority,
            pool_id,
            "partner-1",
            Pubkey::default(),
            partner_1_token_1_account,
        )
        .await;
    let partner_acc = test_env.fetch_account::<Partner>(partner1).await;
    assert_eq!(partner_acc.authority, partner_1_authority.pubkey());
    assert_eq!(&bytes_to_string(&partner_acc.name).unwrap(), "partner-1");
    assert_eq!(partner_acc.pool_state, pool_id);
    assert_eq!(partner_acc.token_0_token_account, Pubkey::default());
    assert_eq!(partner_acc.token_1_token_account, partner_1_token_1_account);

    test_env
        .update_partner(
            &partner_1_authority,
            partner1,
            Some(partner_1_token_0_account),
            None,
        )
        .await;
    let partner_acc = test_env.fetch_account::<Partner>(partner1).await;
    assert_eq!(partner_acc.token_0_token_account, partner_1_token_0_account);
    assert_eq!(partner_acc.token_1_token_account, partner_1_token_1_account);

    let partner2 = test_env
        .initialize_partner(
            &partner_1_authority,
            pool_id,
            "partner-2",
            partner_2_token_0_account,
            partner_2_token_1_account,
        )
        .await;

    // required for admin to add partner to make it valid for the pool
    test_env
        .add_partner(&admin, amm_index, pool_id, partner1)
        .await;
    test_env
        .add_partner(&admin, amm_index, pool_id, partner2)
        .await;

    test_env
        .init_user_pool_liquidity_with_partner(&depositor1, pool_id, Some(partner1))
        .await;
    test_env
        .init_user_pool_liquidity_with_partner(&depositor2, pool_id, Some(partner2))
        .await;
    let depositor1_user_liquidity = derive_user_pool_liquidity(&pool_id, &depositor1.pubkey()).0;
    let depositor2_user_liquidity = derive_user_pool_liquidity(&pool_id, &depositor2.pubkey()).0;

    let depositor1_amount = 200000000;
    test_env
        .deposit(
            &depositor1,
            pool_id,
            amm_index,
            depositor1_amount,
            u64::MAX,
            u64::MAX,
        )
        .await;

    // first swap sequence
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
    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            10000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;

    let pool_state = test_env.fetch_account::<PoolState>(pool_id).await;
    let accumulated_partner_fees_0_after_first_swap_seq = pool_state.partner_protocol_fees_token_0;
    let accumulated_partner_fees_1_after_first_swap_seq = pool_state.partner_protocol_fees_token_1;
    assert_eq_with_copy!(
        accumulated_partner_fees_0_after_first_swap_seq,
        (partner_share_rate as u128
            * (pool_state.protocol_fees_token_0 + accumulated_partner_fees_0_after_first_swap_seq)
                as u128
            / FEE_RATE_DENOMINATOR_VALUE as u128) as u64
    );
    assert_eq_with_copy!(
        accumulated_partner_fees_1_after_first_swap_seq,
        (partner_share_rate as u128
            * (pool_state.protocol_fees_token_1 + accumulated_partner_fees_1_after_first_swap_seq)
                as u128
            / FEE_RATE_DENOMINATOR_VALUE as u128) as u64
    );

    // this deposit triggers a rewards update, awarding a full share of swap1's fees to partner1
    let depositor2_amount = 100000000;
    test_env
        .deposit(
            &depositor2,
            pool_id,
            amm_index,
            depositor2_amount,
            u64::MAX,
            u64::MAX,
        )
        .await;

    // second swap sequence
    test_env
        .swap_base_input(
            &user,
            pool_id,
            amm_index,
            10000000,
            0,
            TradeDirection::ZeroForOne,
        )
        .await;
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
    let pool_state = test_env.fetch_account::<PoolState>(pool_id).await;
    let accumulated_partner_fees_0_after_second_swap_seq = pool_state.partner_protocol_fees_token_0;
    let accumulated_partner_fees_1_after_second_swap_seq = pool_state.partner_protocol_fees_token_1;

    let partner_infos = test_env
        .fetch_account::<PoolPartnerInfos>(pool_partners_key)
        .await;
    let partner1_info = partner_infos
        .infos
        .iter()
        .find(|i| i.partner == partner1)
        .unwrap();
    let partner2_info = partner_infos
        .infos
        .iter()
        .find(|i| i.partner == partner2)
        .unwrap();

    let depositor1_lp_tokens = test_env
        .fetch_account::<UserPoolLiquidity>(depositor1_user_liquidity)
        .await
        .lp_tokens_owned;
    let depositor2_lp_tokens = test_env
        .fetch_account::<UserPoolLiquidity>(depositor2_user_liquidity)
        .await
        .lp_tokens_owned;
    assert_eq_with_copy!(
        partner1_info.lp_token_linked_with_partner as u128,
        depositor1_lp_tokens
    );
    assert_eq_with_copy!(
        partner2_info.lp_token_linked_with_partner as u128,
        depositor2_lp_tokens
    );

    let swap_seq_1_fee_increase_token_0 = accumulated_partner_fees_0_after_first_swap_seq;
    let swap_seq_1_fee_increase_token_1 = accumulated_partner_fees_1_after_first_swap_seq;

    // partner2's deposit must have triggered a rewards update, awarding a full share of swap1's fees to partner1
    assert_eq_with_copy!(
        partner1_info.total_earned_fee_amount_token_0,
        swap_seq_1_fee_increase_token_0
    );
    assert_eq_with_copy!(
        partner1_info.total_earned_fee_amount_token_1,
        swap_seq_1_fee_increase_token_1
    );
    assert_eq_with_copy!(
        partner1_info.last_observed_fee_amount_token_0,
        accumulated_partner_fees_0_after_first_swap_seq
    );
    assert_eq_with_copy!(
        partner1_info.last_observed_fee_amount_token_1,
        accumulated_partner_fees_1_after_first_swap_seq
    );

    // the update in partner2's deposit awards 0 to partner2, because it is done before the deposit amount is added to
    // lp-tokens-linked-with-partner.
    //
    // without earning any fees, partner2's last-observed-values are updated to the accumulated values `after` the first
    // swap, guaranteeing that their calculations never take into account fees earned before their deposit.
    assert_eq_with_copy!(partner2_info.total_earned_fee_amount_token_0, 0);
    assert_eq_with_copy!(partner2_info.total_earned_fee_amount_token_1, 0);
    assert_eq_with_copy!(
        partner1_info.last_observed_fee_amount_token_0,
        accumulated_partner_fees_0_after_first_swap_seq
    );
    assert_eq_with_copy!(
        partner1_info.last_observed_fee_amount_token_1,
        accumulated_partner_fees_1_after_first_swap_seq
    );

    // calculate the unallocated fees(fees from second swap sequence)
    let swap_seq_2_fee_increase_token_0 = accumulated_partner_fees_0_after_second_swap_seq
        - accumulated_partner_fees_0_after_first_swap_seq;
    let swap_seq_2_fee_increase_token_1 = accumulated_partner_fees_1_after_second_swap_seq
        - accumulated_partner_fees_1_after_first_swap_seq;

    // we run the standalone fee-update instruction. now partner1 and partner2 both share the fees from swap2 proportionally
    let partner1_earned_token_0_before_update = partner1_info.total_earned_fee_amount_token_0;
    let partner1_earned_token_1_before_update = partner1_info.total_earned_fee_amount_token_1;
    let partner2_earned_token_0_before_update = partner2_info.total_earned_fee_amount_token_0;
    let partner2_earned_token_1_before_update = partner2_info.total_earned_fee_amount_token_1;

    test_env.update_partner_fees(&user, pool_id).await;
    let partner_infos = test_env
        .fetch_account::<PoolPartnerInfos>(pool_partners_key)
        .await;
    let partner1_info = partner_infos
        .infos
        .iter()
        .find(|i| i.partner == partner1)
        .unwrap();
    let partner2_info = partner_infos
        .infos
        .iter()
        .find(|i| i.partner == partner2)
        .unwrap();
    let total_partner_lp =
        partner1_info.lp_token_linked_with_partner + partner2_info.lp_token_linked_with_partner;
    // this is true because every lp-deposit has been through one of these two partners
    assert_eq_with_copy!(initial_lp_supply + total_partner_lp, pool_state.lp_supply);

    assert_eq_with_copy!(
        partner1_info.total_earned_fee_amount_token_0 - partner1_earned_token_0_before_update,
        ((partner1_info.lp_token_linked_with_partner as u128
            * swap_seq_2_fee_increase_token_0 as u128)
            / total_partner_lp as u128) as u64
    );
    assert_eq_with_copy!(
        partner1_info.total_earned_fee_amount_token_1 - partner1_earned_token_1_before_update,
        ((partner1_info.lp_token_linked_with_partner as u128
            * swap_seq_2_fee_increase_token_1 as u128)
            / total_partner_lp as u128) as u64
    );
    assert_eq_with_copy!(
        partner1_info.last_observed_fee_amount_token_0,
        accumulated_partner_fees_0_after_second_swap_seq
    );
    assert_eq_with_copy!(
        partner1_info.last_observed_fee_amount_token_1,
        accumulated_partner_fees_1_after_second_swap_seq
    );

    assert_eq_with_copy!(
        partner2_info.total_earned_fee_amount_token_0 - partner2_earned_token_0_before_update,
        ((partner2_info.lp_token_linked_with_partner as u128
            * swap_seq_2_fee_increase_token_0 as u128)
            / total_partner_lp as u128) as u64
    );
    assert_eq_with_copy!(
        partner2_info.total_earned_fee_amount_token_1 - partner2_earned_token_1_before_update,
        ((partner2_info.lp_token_linked_with_partner as u128
            * swap_seq_2_fee_increase_token_1 as u128)
            / total_partner_lp as u128) as u64
    );
    assert_eq_with_copy!(
        partner2_info.last_observed_fee_amount_token_0,
        accumulated_partner_fees_0_after_second_swap_seq
    );
    assert_eq_with_copy!(
        partner2_info.last_observed_fee_amount_token_1,
        accumulated_partner_fees_1_after_second_swap_seq
    );
}
