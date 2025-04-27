// Run by using: ANCHOR_WALLET=$HOME/.config/solana/id.json ANCHOR_PROVIDER_URL=rpc  ts-node scripts/migrateAllRewardInfo.ts
const IDL = require("../target/idl/gamma.json");

import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import { Gamma } from "../target/types/gamma";
import { PublicKey } from "@solana/web3.js";
import { readFileSync, writeFileSync } from "fs";

const setUp = () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const idl = IDL as Gamma;
  const program = new Program<Gamma>(idl, anchor.getProvider());
  return program;
};

const migrateAllRewardInfo = async () => {
  const program = setUp();
  const allUserRewardInfo = await program.account.userRewardInfo.all();
  const allRewardInfo = await program.account.rewardInfo.all();

  for (const rewardInfo of allRewardInfo) {
    const allUserRewardInfoForThisRewardInfo = allUserRewardInfo.filter(
      (userRewardInfo) =>
        userRewardInfo.account.rewardInfo === rewardInfo.publicKey
    );

    const amountDistributed = allUserRewardInfoForThisRewardInfo.reduce(
      (acc, userRewardInfo) => acc.add(userRewardInfo.account.totalRewards),
      new BN(0)
    );
    if (
      rewardInfo.account.totalToDisburse.toNumber() ===
      amountDistributed.toNumber()
    ) {
      console.log("Already migrated");
      continue;
    }

    try {
      await program.methods
        .migrateRewardInfo(amountDistributed)
        .accounts({
          authority: program.provider.publicKey,
          poolState: rewardInfo.account.pool,
          rewardInfo: rewardInfo.publicKey,
          ammConfig: new PublicKey(
            "68yDnv1sDzU3L2cek5kNEszKFPaK9yUJaC4ghV5LAXW6"
          ),
        })
        .rpc();
      console.log("Migrated", rewardInfo.publicKey.toString());
    } catch (e) {
      console.log(e);
    }
  }
};

migrateAllRewardInfo();
