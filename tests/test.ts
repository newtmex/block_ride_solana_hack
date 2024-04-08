i  // Configure the client to use the local cluster
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.OctoProgram as anchor.Program<OctoProgram>;
  
mport * as web3 from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import * as splToken from "@solana/spl-token";
import * as anchor from "@coral-xyz/anchor";import type { OctoProgram } from "../target/types/octo_program";

 const web3 = anchor.web3
const usdc = new web3.PublicKey("CY8wKkNH5UwTLpY4gg4eJGYF8fHdyzjMLtn1MopDoN5A");
const wallet = new web3.PublicKey("32UipqSoQJwEeiaFzEnKGqY8fvKnXynsvJQ1pJqeKp9F");
const ata = splToken.getAssociatedTokenAddressSync(usdc,wallet);
console.log(ata)
