// TODO: add transfer fee config to the quote input for token2022,for mints with transfer fee config.
use anchor_lang::AccountDeserialize;
use gamma::states::{AmmConfig, ObservationState, PoolState};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteInput {
    pub source_amount_to_be_swapped: u64,
    pub amm_config_data: Vec<u8>,
    pub pool_state_data: Vec<u8>,
    pub observation_state_data: Vec<u8>,
    pub zero_for_one: bool,
    pub is_invoked_by_signed_segmenter: bool,
}

#[wasm_bindgen(typescript_custom_section)]
const SWAP_RESULT_TYPE: &'static str = r#"
interface SwapResult {
    newSwapSourceAmount: string;
    newSwapDestinationAmount: string;
    sourceAmountSwapped: string;
    destinationAmountSwapped: string;
    dynamicFee: string;
    protocolFee: string;
    fundFee: string;
    dynamicFeeRate: string;
}

interface QuoteInput {
    sourceAmountToBeSwapped: number;
    ammConfigData: Buffer<ArrayBufferLike>;
    poolStateData: Buffer<ArrayBufferLike>;
    observationStateData: Buffer<ArrayBufferLike>;
    zeroForOne: boolean;
    isInvokedBySignedSegmenter: boolean;
}

export function getSwapBaseInputQuoteAmount(val: QuoteInput): SwapResult;
export function getOracleBasedSwapQuoteAmount(val: QuoteInput): SwapResult;
"#;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResult {
    /// New amount of source token
    pub new_swap_source_amount: String,
    /// New amount of destination token
    pub new_swap_destination_amount: String,
    /// Amount of source token swapped (includes fees)
    pub source_amount_swapped: String,
    /// Amount of destination token swapped
    pub destination_amount_swapped: String,
    /// Dynamic fee charged for trade
    pub dynamic_fee: String,
    /// Amount of source tokens going to protocol
    pub protocol_fee: String,
    /// Amount of source tokens going to protocol team
    pub fund_fee: String,
    /// Dynamic fee rate
    pub dynamic_fee_rate: String,
}

#[wasm_bindgen(js_name = "getSwapBaseInputQuoteAmount", skip_typescript)]
pub fn get_swap_base_input_quote_amount(val: JsValue) -> JsValue {
    let quote_input: QuoteInput =
        serde_wasm_bindgen::from_value(val).expect("Failed to deserialize quote input");
    let pool_state: PoolState =
        PoolState::try_deserialize(&mut quote_input.pool_state_data.as_ref())
            .expect("Failed to deserialize pool state");
    let amm_config: AmmConfig =
        AmmConfig::try_deserialize(&mut quote_input.amm_config_data.as_ref())
            .expect("Failed to deserialize amm config");
    let observation_state: ObservationState =
        ObservationState::try_deserialize(&mut quote_input.observation_state_data.as_ref())
            .expect("Failed to deserialize observation state");

    let current_time_in_unix_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get current time")
        .as_secs();

    let (swap_source_amount, swap_destination_amount) = if quote_input.zero_for_one {
        (
            pool_state.token_0_vault_amount,
            pool_state.token_1_vault_amount,
        )
    } else {
        (
            pool_state.token_1_vault_amount,
            pool_state.token_0_vault_amount,
        )
    };

    let swap_result = gamma::curve::CurveCalculator::swap_base_input(
        u128::from(quote_input.source_amount_to_be_swapped),
        u128::from(swap_source_amount),
        u128::from(swap_destination_amount),
        &amm_config,
        &pool_state,
        current_time_in_unix_timestamp,
        &observation_state,
        quote_input.is_invoked_by_signed_segmenter,
    )
    .expect("Failed to calculate swap result");

    let swap_result_js = SwapResult {
        new_swap_source_amount: swap_result.new_swap_source_amount.to_string(),
        new_swap_destination_amount: swap_result.new_swap_destination_amount.to_string(),
        source_amount_swapped: swap_result.source_amount_swapped.to_string(),
        destination_amount_swapped: swap_result.destination_amount_swapped.to_string(),
        dynamic_fee: swap_result.dynamic_fee.to_string(),
        protocol_fee: swap_result.protocol_fee.to_string(),
        fund_fee: swap_result.fund_fee.to_string(),
        dynamic_fee_rate: swap_result.dynamic_fee_rate.to_string(),
    };
    serde_wasm_bindgen::to_value(&swap_result_js).expect("Failed to serialize swap result")
}

#[wasm_bindgen(js_name = "getOracleBasedSwapQuoteAmount", skip_typescript)]
pub fn get_oracle_based_swap_quote_amount(val: JsValue) -> JsValue {
    let quote_input: QuoteInput =
        serde_wasm_bindgen::from_value(val).expect("Failed to deserialize quote input");
    let pool_state: PoolState =
        PoolState::try_deserialize(&mut quote_input.pool_state_data.as_ref())
            .expect("Failed to deserialize pool state");
    let amm_config: AmmConfig =
        AmmConfig::try_deserialize(&mut quote_input.amm_config_data.as_ref())
            .expect("Failed to deserialize amm config");
    let observation_state: ObservationState =
        ObservationState::try_deserialize(&mut quote_input.observation_state_data.as_ref())
            .expect("Failed to deserialize observation state");

    let current_time_in_unix_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Failed to get current time")
        .as_secs();

    let (swap_source_amount, swap_destination_amount) = if quote_input.zero_for_one {
        (
            pool_state.token_0_vault_amount,
            pool_state.token_1_vault_amount,
        )
    } else {
        (
            pool_state.token_1_vault_amount,
            pool_state.token_0_vault_amount,
        )
    };

    let swap_result = gamma::curve::OracleBasedSwapCalculator::swap_base_input(
        u128::from(quote_input.source_amount_to_be_swapped),
        u128::from(swap_source_amount),
        u128::from(swap_destination_amount),
        &amm_config,
        &pool_state,
        current_time_in_unix_timestamp,
        &observation_state,
        quote_input.is_invoked_by_signed_segmenter,
    )
    .expect("Failed to calculate swap result");

    let swap_result_js = SwapResult {
        new_swap_source_amount: swap_result.new_swap_source_amount.to_string(),
        new_swap_destination_amount: swap_result.new_swap_destination_amount.to_string(),
        source_amount_swapped: swap_result.source_amount_swapped.to_string(),
        destination_amount_swapped: swap_result.destination_amount_swapped.to_string(),
        dynamic_fee: swap_result.dynamic_fee.to_string(),
        protocol_fee: swap_result.protocol_fee.to_string(),
        fund_fee: swap_result.fund_fee.to_string(),
        dynamic_fee_rate: swap_result.dynamic_fee_rate.to_string(),
    };
    serde_wasm_bindgen::to_value(&swap_result_js).expect("Failed to serialize swap result")
}
