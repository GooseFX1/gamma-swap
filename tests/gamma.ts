import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Gamma } from "../target/types/gamma";
// import IDL from "../target/idl/gamma.json";
import {
  ComputeBudgetInstruction,
  ComputeBudgetProgram,
  PublicKey,
} from "@solana/web3.js";
import bs58 from "bs58";
import { Keypair } from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  NATIVE_MINT,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { decode, encode } from "@coral-xyz/anchor/dist/cjs/utils/bytes/hex";
const IDL = require("../target/idl/gamma.json");
import {
  createAssociatedTokenAccountInstruction,
  createSyncNativeInstruction,
  getAccount,
} from "@solana/spl-token";
import {
  clusterApiUrl,
  Connection,
  LAMPORTS_PER_SOL,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";

describe("gamma", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());
  const idl = IDL as Gamma;
  const program = new Program<Gamma>(
    idl,
    "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT",
    anchor.getProvider()
  );

  // const program = anchor.workspace.Gamma as Program<Gamma>;

  // it("Is initialized!", async () => {
  //   const idl = IDL as Gamma;
  //   idl.address = "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT" as any;
  //   const program = new Program<Gamma>(idl, anchor.getProvider());
  //   // // Add your test here.
  //   // const tx = await program.methods.initialize().rpc();
  //   // console.log("Your transaction signature", tx);

  //   const amm = await program.account.ammConfig.all();
  //   console.log(`Program initialized ${JSON.stringify(amm)}`);
  // });

  // it("Should create pool", async () => {

  //   const idl = IDL as Gamma;
  //   idl.address = "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT" as any;
  //   const program = new Program<Gamma>(idl, anchor.getProvider());

  // const amm = await program.account.ammConfig.all();

  // console.log("WALler")
  // try {
  //   await program.methods
  //     .initialize(
  //       new anchor.BN(0.4 * 1e9),
  //       new anchor.BN(62.01 * 1e6),
  //       new anchor.BN(0)
  //     )
  //     .accounts({

  //     })
  //     .rpc();
  // } catch (e) {
  //   console.log(e);
  // }
  // });

  it("Should swap 1->0", async () => {
    const pool_id = new PublicKey(
      "Hjm1F98vgVdN7Y9L46KLqcZZWyTKS9tj9ybYKJcXnSng"
    );
    const idl = IDL as Gamma;
    const program = new Program<Gamma>(
      idl,
      "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT",
      anchor.getProvider()
    );
    const amm = await program.account.ammConfig.all();
    const pool_state = await program.account.poolState.fetch(pool_id);
    console.log(`Pool state ${JSON.stringify(pool_state)}`);

    // const associatedTokenAccount = await getAssociatedTokenAddress(
    //   NATIVE_MINT,
    //   anchor.Wallet.local().publicKey
    // );

    // // Create token account to hold your wrapped SOL
    // const ataTransaction = new Transaction().add(
    //   createAssociatedTokenAccountInstruction(
    //     anchor.Wallet.local().publicKey,
    //     associatedTokenAccount,
    //     anchor.Wallet.local().publicKey,
    //     NATIVE_MINT
    //   )
    // );
    // await sendAndConfirmTransaction(
    //   program.provider.connection,
    //   ataTransaction,
    //   [anchor.Wallet.local().payer]
    // );
    // anchor.Wallet.local().signTransaction(ataTransaction);
    // await program.provider.connection.sendTransaction(ataTransaction, [
    //   anchor.Wallet.local().payer,
    // ]);
    const encodeSeedString = (seedString: string) =>
      Buffer.from(anchor.utils.bytes.utf8.encode(seedString));

    const usdMint = new PublicKey(
      "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    );
    try {
      await program.methods
        .swapBaseInput(new anchor.BN(0.0001 * 1e6), new anchor.BN(0))
        .accounts({
          ammConfig: amm[0].publicKey,
          poolState: pool_id,
          inputVault: pool_state.token1Vault,
          outputVault: pool_state.token0Vault,
          observationState: pool_state.observationKey,
          payer: anchor.Wallet.local().publicKey,
          inputTokenAccount: await getAssociatedTokenAddress(
            usdMint,
            anchor.Wallet.local().publicKey
          ),
          outputTokenAccount: await getAssociatedTokenAddress(
            NATIVE_MINT,
            anchor.Wallet.local().publicKey
          ),
          inputTokenMint: usdMint,
          outputTokenMint: NATIVE_MINT,
          inputTokenProgram: TOKEN_PROGRAM_ID,
          outputTokenProgram: TOKEN_PROGRAM_ID,
          authority: PublicKey.findProgramAddressSync(
            [encodeSeedString("vault_and_lp_mint_auth_seed")],
            program.programId
          )[0],
        })
        .preInstructions([
          ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 1000000 }),
        ])
        .rpc();
    } catch (e) {
      console.log(e);
    }
  });

  // it("Should swap 0->1", async () => {
  //   const pool_id = new PublicKey(
  //     "Hjm1F98vgVdN7Y9L46KLqcZZWyTKS9tj9ybYKJcXnSng"
  //   );
  //   const idl = IDL as Gamma;
  //   const program = new Program<Gamma>(
  //     idl,
  //     "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT",
  //     anchor.getProvider()
  //   );
  //   const amm = await program.account.ammConfig.all();
  //   const pool_state = await program.account.poolState.fetch(pool_id);
  //   console.log(`Pool state ${JSON.stringify(pool_state)}`);

  //   const usdMint = new PublicKey(
  //     "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  //   );
  //   try {
  //     await program.methods
  //       .swapBaseInput(new anchor.BN(0.0001 * 1e6), new anchor.BN(0))
  //       .accounts({
  //         ammConfig: amm[0].publicKey,
  //         poolState: pool_id,
  //         inputVault: pool_state.token0Vault,
  //         outputVault: pool_state.token1Vault,
  //         observationState: pool_state.observationKey,
  //         payer: anchor.Wallet.local().publicKey,
  //         inputTokenAccount: await getAssociatedTokenAddress(
  //           NATIVE_MINT,
  //           anchor.Wallet.local().publicKey
  //         ),
  //         outputTokenAccount: await getAssociatedTokenAddress(
  //           usdMint,
  //           anchor.Wallet.local().publicKey
  //         ),
  //         inputTokenMint: NATIVE_MINT,
  //         outputTokenMint: usdMint,
  //         inputTokenProgram: TOKEN_PROGRAM_ID,
  //         outputTokenProgram: TOKEN_PROGRAM_ID,

  //       })
  //       .rpc();
  //   } catch (e) {
  //     console.log(e);
  //   }
  // });
});
