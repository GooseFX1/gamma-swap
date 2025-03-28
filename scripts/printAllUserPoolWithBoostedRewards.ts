// Run by using: ANCHOR_WALLET=$HOME/.config/solana/id.json ANCHOR_PROVIDER_URL=rpc ts-node scripts/printAllUserPoolWithBoostedRewards.ts
const IDL = require("../target/idl/gamma.json");

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Gamma } from "../target/types/gamma";

const setUp = () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const idl = IDL as Gamma;
  const program = new Program<Gamma>(idl, anchor.getProvider());
  return program;
};

const main = async () => {
  const program = setUp();
  const userPool = await program.account.userRewardInfo.all();
  console.log(userPool);
};

main();
