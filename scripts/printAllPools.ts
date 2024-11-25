// Run by using: ANCHOR_WALLET=$HOME/.config/solana/id.json ANCHOR_PROVIDER_URL=https://rpc.credix.finance  ts-node scripts/printAllPools.ts
const IDL = require("../target/idl/gamma.json");

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Gamma } from "../target/types/gamma";

const setUp = () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const idl = IDL as Gamma;
  const program = new Program<Gamma>(
    idl,
    "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT",
    anchor.getProvider()
  );
  return program;
};

const printAllPools = async () => {
  const program = setUp();
  const pools = await program.account.poolState.all();
  console.log(JSON.stringify(pools));
};

const printUserPoolLiquidity = async () => {
  const program = setUp();
  const userPools = await program.account.userPoolLiquidity.all();
  console.log(JSON.stringify(userPools));
};

// printAllPools();
printUserPoolLiquidity();
