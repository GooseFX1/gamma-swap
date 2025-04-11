// Run by using: ANCHOR_WALLET=$HOME/.config/solana/id.json ANCHOR_PROVIDER_URL=https://yearling-adorne-fast-mainnet.helius-rpc.com/ npx ts-node scripts/printAllUserPoolWithBoostedRewards.ts
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
  const userPool = await program.account.userRewardInfo.fetch(
    "9xz18HNTXNiV96EiWjFjowAfc3qxpo767r7oWNkFM2XE"
  );
  const rewardInfo = await program.account.rewardInfo.fetch(
    userPool.rewardInfo
  );
  console.log(rewardInfo.endRewardsAt.toNumber());
  console.log(userPool.rewardsLastCalculatedAt.toNumber());
};

main();
