// Run by using: ANCHOR_WALLET=$HOME/.config/solana/id.json ANCHOR_PROVIDER_URL=rpc  ts-node scripts/migrateAllPool.ts
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
// before running create this empty file:
const FILE_PATH = "scripts/poolDataMigrationBoostedRewards.json";

export const encodeSeedString = (seedString: string) =>
  Buffer.from(anchor.utils.bytes.utf8.encode(seedString));

export const readStorage = () => {
  const transactionFile = readFileSync(FILE_PATH);
  if (transactionFile.toString() != "") {
    return JSON.parse(transactionFile.toString()) as string[];
  }
  return JSON.parse("[]") as string[];
};

export const addAddressOfMigratedAccount = (
  data: string[],
  poolAddress: PublicKey
) => {
  data.push(poolAddress.toString());
  writeFileSync(FILE_PATH, JSON.stringify(data, null, 2));
};

const migrateAllUsers = async () => {
  const program = setUp();
  const userPoolLiquidityAccounts =
    await program.account.userPoolLiquidity.all();

  const data = readStorage();
  for (const userPoolLiquidity of userPoolLiquidityAccounts) {
    if (data.includes(userPoolLiquidity.publicKey.toString())) {
      console.log("Already migrated");
      continue;
    }

    try {
      await program.methods
        .migration()
        .accounts({
          signer: program.provider.publicKey,
          poolState: userPoolLiquidity.account.poolState,
          owner: userPoolLiquidity.account.user,
        })
        .rpc();
      addAddressOfMigratedAccount(data, userPoolLiquidity.publicKey);
    } catch (e) {
      console.log(e);
    }
  }
};

migrateAllUsers();
