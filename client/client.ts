import * as web3 from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import * as splToken from "@solana/spl-token";
import * as anchor from "@coral-xyz/anchor";
import type { OctoProgram } from "../target/types/octo_program";

// Configure the client to use the local cluster
anchor.setProvider(anchor.AnchorProvider.env());

const program = anchor.workspace.OctoProgram as anchor.Program<OctoProgram>;

 const web3 = anchor.web3
const usdc = new web3.PublicKey("F4SXWDnCM9xSBGK7KrjTyHDDrmSwy4MSFd8HaASPfEj5");
const wallet = new web3.PublicKey("32UipqSoQJwEeiaFzEnKGqY8fvKnXynsvJQ1pJqeKp9F");
const ata = splToken.getAssociatedTokenAddressSync(usdc,wallet);
console.log(ata.toBase58())

