import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Gamma } from "../target/types/gamma";
// import IDL from "../target/idl/gamma.json";
import { PublicKey } from "@solana/web3.js";
const IDL = require("../target/idl/gamma.json");
describe("gamma", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Gamma as Program<Gamma>;

  it("Is initialized!", async () => {
    // // Add your test here.
    // const tx = await program.methods.initialize().rpc();
    // console.log("Your transaction signature", tx);
    const idl = IDL as Gamma;
    idl.address = "GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT" as any;
    const program = new Program<Gamma>(idl, anchor.getProvider());
    const amm = await program.account.ammConfig.all();
    console.log(`Program initialized ${JSON.stringify(amm)}`);
  });
});
