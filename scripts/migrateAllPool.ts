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
  const program = new Program<Gamma>(
    idl,
    "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT",
    anchor.getProvider()
  );
  return program;
};
// before runnning create this empty file:
const FILE_PATH = "scripts/poolDataMigration.json";

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

const migrateAllPool = async () => {
  const program = setUp();
  const pools = await program.account.poolState.all();

  const data = readStorage();
  for (const pool of pools) {
    if (data.includes(pool.publicKey.toString())) {
      console.log("Already migrated");
      continue;
    }

    try {
      await program.methods
        .updatePool(10, new BN(10))
        .accounts({
          token0Vault: pool.account.token0Vault,
          token1Vault: pool.account.token1Vault,
          poolState: pool.publicKey,
          authority: program.provider.publicKey,
        })
        .rpc();
      addAddressOfMigratedAccount(data, pool.publicKey);
    } catch (e) {
      console.log(e);
    }
  }
};
