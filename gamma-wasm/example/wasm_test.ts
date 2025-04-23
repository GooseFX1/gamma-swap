// This is a file to test if the swap base input quote amount is working correctly.
// With wasm
// ANCHOR_WALLET=$HOME/.config/solana/id.json ANCHOR_PROVIDER_URL=https://api.mainnet-beta.solana.com npx ts-node gamma-wasm/example/wasm_test.ts
import fs from "node:fs";

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Gamma } from "../../target/types/gamma";
import { PublicKey } from "@solana/web3.js";
import * as wasm from "../pkg/gamma_wasm";

const IDL = require("../../target/idl/gamma.json");

const testSwapBaseInput = async () => {
  wasm.solana_program_init();

  anchor.setProvider(anchor.AnchorProvider.env());
  const idl = IDL as Gamma;
  const program = new Program<Gamma>(idl, anchor.getProvider());
  const sol_usdc_pol_pk = new PublicKey(
    "Hjm1F98vgVdN7Y9L46KLqcZZWyTKS9tj9ybYKJcXnSng"
  );
  const poolState = await program.account.poolState.fetch(sol_usdc_pol_pk);

  const pool_state_data = await program.provider.connection.getAccountInfo(
    sol_usdc_pol_pk
  );
  const amm_config_data = await program.provider.connection.getAccountInfo(
    poolState.ammConfig
  );
  const observation_state_data =
    await program.provider.connection.getAccountInfo(poolState.observationKey);

  const quoteInput: wasm.QuoteInput = {
    sourceAmountToBeSwapped: 1000,
    ammConfigData: amm_config_data.data,
    poolStateData: pool_state_data.data,
    observationStateData: observation_state_data.data,
    zeroForOne: true,
    isInvokedBySignedSegmenter: false,
  };

  const result = wasm.getSwapBaseInputQuoteAmount(quoteInput);
  console.log(result);
};

const testOracleBasedSwapBaseInput = async () => {
  wasm.solana_program_init();

  anchor.setProvider(anchor.AnchorProvider.env());
  const idl = IDL as Gamma;
  const program = new Program<Gamma>(idl, anchor.getProvider());
  const sol_usdc_pol_pk = new PublicKey(
    "Hjm1F98vgVdN7Y9L46KLqcZZWyTKS9tj9ybYKJcXnSng"
  );
  const poolState = await program.account.poolState.fetch(sol_usdc_pol_pk);

  const pool_state_data = await program.provider.connection.getAccountInfo(
    sol_usdc_pol_pk
  );
  const amm_config_data = await program.provider.connection.getAccountInfo(
    poolState.ammConfig
  );
  const observation_state_data =
    await program.provider.connection.getAccountInfo(poolState.observationKey);

  const quoteInput: wasm.QuoteInput = {
    sourceAmountToBeSwapped: 1000,
    ammConfigData: amm_config_data.data,
    poolStateData: pool_state_data.data,
    observationStateData: observation_state_data.data,
    zeroForOne: true,
    isInvokedBySignedSegmenter: false,
  };

  const result = wasm.getOracleBasedSwapQuoteAmount(quoteInput);
  console.log(result);
};

testSwapBaseInput();
testOracleBasedSwapBaseInput();
