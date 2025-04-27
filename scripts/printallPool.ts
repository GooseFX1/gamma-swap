// Run by using: ANCHOR_WALLET=$HOME/.config/solana/id.json ANCHOR_PROVIDER_URL=https://yearling-adorne-fast-mainnet.helius-rpc.com/ npx ts-node scripts/printallPool.ts
const IDL = require("../target/idl/gamma.json");

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Gamma } from "../target/types/gamma";
import { PublicKey } from "@solana/web3.js";

const setUp = () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const idl = IDL as Gamma;
  const program = new Program<Gamma>(idl, anchor.getProvider());
  return program;
};

const main = async () => {
  const program = setUp();
  const pools = await program.account.poolState.all();
  console.log(pools);
  for (const pool of pools) {
    if (!pool.account.padding.every((p) => p.eq(new anchor.BN(0)))) {
      console.log(pool.publicKey.toBase58());
    }
    if (!pool.account.padding3.every((p) => p === 0)) {
      console.log(pool.publicKey.toBase58());
    }
  }
};

main();
