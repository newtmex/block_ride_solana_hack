import BN from "bn.js";
import * as web3 from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import * as splToken from "@solana/spl-token";
import { OctoProgram } from "../target/types/octo_program";
import type { OctoProgram } from "../target/types/octo_program";

// program.account.pool
describe("octo-program", () => {
  // Configure the client to use the local cluster
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.OctoProgram as anchor.Program<OctoProgram>;
  
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const web3 = anchor.web3;
  const payer = provider.wallet as anchor.Wallet;
  const program = anchor.workspace.OctoProgram as Program<OctoProgram>;
  const usdcMint = new web3.PublicKey(
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  );
  const metadataProgram = new web3.PublicKey(
    "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
  );

  const fakeAuthority = web3.Keypair.generate();
  const distAuthority = web3.Keypair.generate();
  const reference = web3.Keypair.generate();

  const [poolAddr] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("pool"), reference.publicKey.toBytes()],
    program.programId
  );
  const [mintAddr] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("mint"), poolAddr.toBytes()],
    program.programId
  );
  const [distAddr] = web3.PublicKey.findProgramAddressSync(
    [Buffer.from("distribution"), poolAddr.toBytes()],
    program.programId
  );

  beforeAll(async () => {
    // Mint USDT to payer
    const tokenAccount = await splToken.getOrCreateAssociatedTokenAccount(
      provider.connection,
      payer.payer,
      usdcMint,
      payer.publicKey
    );

    await splToken.mintTo(
      provider.connection,
      payer.payer,
      usdcMint,
      tokenAccount.address,
      payer.publicKey,
      1_000_000e6 // 1,000,000 USDC
    );
  });

  it("Create Pool", async () => {
    const [metadataAddr] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), metadataProgram.toBytes(), mintAddr.toBytes()],
      metadataProgram
    );

    const creatorMintAccount = splToken.getAssociatedTokenAddressSync(
      mintAddr,
      payer.publicKey
    );
    const creatorUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      payer.publicKey
    );
    const poolUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      poolAddr,
      true
    );

    await program.methods
      .createPool(
        payer.publicKey, // authority
        new anchor.BN(100_000e6), // seed
        new anchor.BN(100), // shares
        new anchor.BN(3_000e6) // deposit
      )
      .accounts({
        creator: payer.publicKey,
        reference: reference.publicKey,
        pool: poolAddr,
        mint: mintAddr,
        creatorMintAccount,
        usdcMint,
        creatorUsdcAccount,
        poolUsdcAccount,
        metadata: metadataAddr,
        metadataProgram,
      })
      .signers([payer.payer, reference])
      .rpc();

    const poolAccount = await program.account.pool.fetch(poolAddr);
    expect(poolAccount.creator.equals(payer.publicKey)).toBe(true);
    expect(poolAccount.authority.equals(payer.publicKey)).toBe(true);
    expect(poolAccount.reference.equals(reference.publicKey)).toBe(true);
    expect(poolAccount.mint.equals(mintAddr)).toBe(true);
    expect(poolAccount.seed.eq(new anchor.BN(100_000e6))).toBe(true);
    expect(poolAccount.shares.eq(new anchor.BN(100))).toBe(true);
    expect(poolAccount.minted.eq(new anchor.BN(3))).toBe(true);

    const poolUsdcBalance = await provider.connection.getTokenAccountBalance(
      poolUsdcAccount
    );
    expect(poolUsdcBalance.value.uiAmount).toBe(3_000);

    const shares = await provider.connection.getTokenAccountBalance(
      creatorMintAccount
    );
    expect(shares.value.uiAmount).toBe(3);
  });

  describe("Buy Shares", () => {
    const buyerUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      payer.publicKey
    );
    const poolUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      poolAddr,
      true
    );
    const buyerMintAccount = splToken.getAssociatedTokenAddressSync(
      mintAddr,
      payer.publicKey
    );

    it("Buy 3 more shares", async () => {
      await program.methods
        .buyShares(new anchor.BN(3))
        .accounts({
          buyer: payer.publicKey,
          pool: poolAddr,
          mint: mintAddr,
          buyerMintAccount,
          usdcMint,
          buyerUsdcAccount,
          poolUsdcAccount,
        })
        .signers([payer.payer])
        .rpc();

      const poolAccount = await program.account.pool.fetch(poolAddr);
      expect(poolAccount.minted.eq(new anchor.BN(6))).toBe(true);

      const poolUsdcBalance = await provider.connection.getTokenAccountBalance(
        poolUsdcAccount
      );
      expect(poolUsdcBalance.value.uiAmount).toBe(6_000);

      const shares = await provider.connection.getTokenAccountBalance(
        buyerMintAccount
      );
      expect(shares.value.uiAmount).toBe(6);
    });

    it("Should fail when exceeds available shares", async () => {
      expect(
        program.methods
          .buyShares(new anchor.BN(100))
          .accounts({
            buyer: payer.publicKey,
            pool: poolAddr,
            mint: mintAddr,
            buyerMintAccount,
            usdcMint,
            buyerUsdcAccount,
            poolUsdcAccount,
          })
          .signers([payer.payer])
          .rpc()
      ).rejects.toThrow("Exceeds available shares");
    });
  });

  describe("Distribution", () => {
    let test = 0;
    const distributionUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      distAddr,
      true
    );

    beforeAll(async () => {
      // Create distribution account
      await splToken.getOrCreateAssociatedTokenAccount(
        provider.connection,
        payer.payer,
        usdcMint,
        distAddr,
        true
      );

      // Airdrop 1 SOL to fake authority
      await provider.connection.requestAirdrop(
        fakeAuthority.publicKey,
        web3.LAMPORTS_PER_SOL
      );
    });

    beforeEach(async () => {
      test++;
      if (test === 2) {
        // Mint USDC to distribution account
        await splToken.mintTo(
          provider.connection,
          payer.payer,
          usdcMint,
          distributionUsdcAccount,
          payer.publicKey,
          10_000e6
        );
      }
    });

    it("Should fail when insufficient distribution USDC balance", async () => {
      expect(
        program.methods
          .distribute(
            new anchor.BN(10_000e6) // 10,000 USDC
          )
          .accounts({
            authority: payer.publicKey,
            distributionAuthority: distAuthority.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            usdcMint,
            distributionUsdcAccount,
          })
          .signers([payer.payer, distAuthority])
          .rpc()
      ).rejects.toThrow("Insufficient distribution USDC balance");
    });
    program.account.poolCreatorData;
    it("Should fail when not the authority", async () => {
      expect(
        program.methods
          .distribute(
            new anchor.BN(10_000e6) // 10,000 USDC
          )
          .accounts({
            authority: fakeAuthority.publicKey,
            distributionAuthority: distAuthority.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            usdcMint,
            distributionUsdcAccount,
          })
          .signers([fakeAuthority, distAuthority])
          .rpc()
      ).rejects.toThrow("An address constraint was violated");
    });

    it("Distribute 10,000 USDC", async () => {
      await program.methods
        .distribute(
          new anchor.BN(10_000e6) // 10,000 USDC
        )
        .accounts({
          authority: payer.publicKey,
          distributionAuthority: distAuthority.publicKey,
          pool: poolAddr,
          distribution: distAddr,
          usdcMint,
          distributionUsdcAccount,
        })
        .signers([payer.payer, distAuthority])
        .rpc();

      const distAcc = await program.account.distribution.fetch(distAddr);
      expect(distAcc.pool.equals(poolAddr)).toBe(true);
      expect(distAcc.authority.equals(distAuthority.publicKey)).toBe(true);
      expect(distAcc.rewards.eq(new anchor.BN(10_000e6))).toBe(true);
    });

    afterAll(async () => {
      // Send SOL from fake authority
      const balance = await provider.connection.getBalance(
        fakeAuthority.publicKey
      );
      await provider.sendAndConfirm(
        new web3.Transaction().add(
          web3.SystemProgram.transfer({
            fromPubkey: fakeAuthority.publicKey,
            toPubkey: payer.publicKey,
            lamports: balance,
          })
        ),
        [fakeAuthority]
      );
    });
  });

  describe("Claim Rewards", () => {
    const holderMintAccount = splToken.getAssociatedTokenAddressSync(
      mintAddr,
      payer.publicKey
    );
    const distributionUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      distAddr,
      true
    );
    const holderUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      payer.publicKey
    );

    it("Should fail when distribution authority is not valid", async () => {
      const fakeAuthority = web3.Keypair.generate();
      expect(
        program.methods
          .claimRewards(
            new anchor.BN(1_000e6) // 1,000 USDC
          )
          .accounts({
            holder: payer.publicKey,
            authority: fakeAuthority.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            mint: mintAddr,
            holderMintAccount,
            usdcMint,
            distributionUsdcAccount,
            holderUsdcAccount,
          })
          .signers([payer.payer, fakeAuthority])
          .rpc()
      ).rejects.toThrow("An address constraint was violated");
    });

    it("Should fail when insufficient rewards", async () => {
      expect(
        program.methods
          .claimRewards(
            new anchor.BN(100_000e6) // 100,000 USDC
          )
          .accounts({
            holder: payer.publicKey,
            authority: distAuthority.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            mint: mintAddr,
            holderMintAccount,
            usdcMint,
            distributionUsdcAccount,
            holderUsdcAccount,
          })
          .signers([payer.payer, distAuthority])
          .rpc()
      ).rejects.toThrow("Insufficient distribution USDC balance");
    });

    it("Claim 1,000 USDC rewards", async () => {
      // Holders USDC balance before claiming rewards
      const $_before = await provider.connection.getTokenAccountBalance(
        holderUsdcAccount
      );

      // Claim rewards
      await program.methods
        .claimRewards(
          new anchor.BN(1_000e6) // 1,000 USDC
        )
        .accounts({
          holder: payer.publicKey,
          authority: distAuthority.publicKey,
          pool: poolAddr,
          distribution: distAddr,
          mint: mintAddr,
          holderMintAccount,
          usdcMint,
          distributionUsdcAccount,
          holderUsdcAccount,
        })
        .signers([payer.payer, distAuthority])
        .rpc();

      const distAcc = await program.account.distribution.fetch(distAddr);
      expect(distAcc.claimed.eq(new anchor.BN(1_000e6))).toBe(true);

      // Holders USDC balance after claiming rewards
      const $_after = await provider.connection.getTokenAccountBalance(
        holderUsdcAccount
      );
      const balance = $_after.value.uiAmount - $_before.value.uiAmount;
      expect(balance).toBe(1_000);
    });

    it("Should fail when holder is not a previous holder", async () => {
      const fakeHolder = web3.Keypair.generate();
      const holderMintAccount = splToken.getAssociatedTokenAddressSync(
        mintAddr,
        fakeHolder.publicKey
      );
      const holderUsdcAccount = splToken.getAssociatedTokenAddressSync(
        usdcMint,
        fakeHolder.publicKey
      );

      expect(
        program.methods
          .claimRewards(
            new anchor.BN(1_000e6) // 1,000 USDC
          )
          .accounts({
            holder: fakeHolder.publicKey,
            authority: distAuthority.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            mint: mintAddr,
            holderMintAccount,
            usdcMint,
            distributionUsdcAccount,
            holderUsdcAccount,
          })
          .signers([fakeHolder, distAuthority])
          .rpc()
      ).rejects.toThrow(
        "The program expected this account to be already initialized"
      );
    });
  });

  describe("Withdraw From Pool", () => {
    let test = 0;
    const foo = web3.Keypair.generate();

    const poolUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      poolAddr,
      true
    );
    const toUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      foo.publicKey
    );

    beforeAll(async () => {
      // Airdrop 1 SOL to foo
      await provider.connection.confirmTransaction(
        await provider.connection.requestAirdrop(
          foo.publicKey,
          web3.LAMPORTS_PER_SOL
        ),
        "confirmed"
      );

      // Create ATA
      await splToken.createAssociatedTokenAccount(
        provider.connection,
        foo,
        usdcMint,
        foo.publicKey
      );
    });

    beforeEach(async () => {
      test++;
      if (test === 3) {
        const pool = await program.account.pool.fetch(poolAddr);

        // Complete seed rounds
        await program.methods
          .buyShares(pool.shares.sub(pool.minted))
          .accounts({
            buyer: payer.publicKey,
            pool: poolAddr,
            mint: mintAddr,
            buyerMintAccount: splToken.getAssociatedTokenAddressSync(
              mintAddr,
              payer.publicKey
            ),
            usdcMint,
            buyerUsdcAccount: splToken.getAssociatedTokenAddressSync(
              usdcMint,
              payer.publicKey
            ),
            poolUsdcAccount,
          })
          .signers([payer.payer])
          .rpc();
      }
    });

    it("Should fail when the authority is not valid", async () => {
      expect(
        program.methods
          .withdrawFromPool(new anchor.BN(1))
          .accounts({
            authority: fakeAuthority.publicKey,
            pool: poolAddr,
            usdcMint,
            poolUsdcAccount,
            toUsdcAccount,
          })
          .signers([fakeAuthority])
          .rpc()
      ).rejects.toThrow("An address constraint was violated");
    });

    it("Should fail when the seed rounds are not completed", async () => {
      expect(
        program.methods
          .withdrawFromPool(new anchor.BN(1))
          .accounts({
            authority: payer.publicKey,
            pool: poolAddr,
            usdcMint,
            poolUsdcAccount,
            toUsdcAccount,
          })
          .signers([payer.payer])
          .rpc()
      ).rejects.toThrow("Seed rounds are not completed");
    });

    it("Should fail when shares to withdraw exceed available pool USDC balance", async () => {
      const pool = await program.account.pool.fetch(poolAddr);
      expect(
        program.methods
          .withdrawFromPool(pool.shares.add(new anchor.BN(1)))
          .accounts({
            authority: payer.publicKey,
            pool: poolAddr,
            usdcMint,
            poolUsdcAccount,
            toUsdcAccount,
          })
          .signers([payer.payer])
          .rpc()
      ).rejects.toThrow("Insufficient pool USDC balance");
    });

    it("Withdraw from pool", async () => {
      const pool = await program.account.pool.fetch(poolAddr);

      // Withdraw from pool
      await program.methods
        .withdrawFromPool(pool.shares)
        .accounts({
          authority: payer.publicKey,
          pool: poolAddr,
          usdcMint,
          poolUsdcAccount,
          toUsdcAccount,
        })
        .signers([payer.payer])
        .rpc();

      const poolUsdcBalance = await provider.connection.getTokenAccountBalance(
        poolUsdcAccount
      );
      expect(poolUsdcBalance.value.uiAmount).toBe(0);

      const toUsdcBalance = await provider.connection.getTokenAccountBalance(
        toUsdcAccount
      );
      expect(toUsdcBalance.value.uiAmountString).toBe(
        pool.shares
          .mul(new anchor.BN(1_000e6))
          .div(new anchor.BN(1e6))
          .toString()
      );
    });
  });

  describe("Close Pool", () => {
    const distAuthority = web3.Keypair.generate();
    const reference = web3.Keypair.generate();

    const [poolAddr] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("pool"), reference.publicKey.toBytes()],
      program.programId
    );
    const [distAddr] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("distribution"), poolAddr.toBytes()],
      program.programId
    );
    const [mintAddr] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("mint"), poolAddr.toBytes()],
      program.programId
    );
    const [metadataAddr] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), metadataProgram.toBytes(), mintAddr.toBytes()],
      metadataProgram
    );

    const creatorMintAccount = splToken.getAssociatedTokenAddressSync(
      mintAddr,
      payer.publicKey
    );
    const creatorUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      payer.publicKey
    );
    const poolUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      poolAddr,
      true
    );
    const distributionUsdcAccount = splToken.getAssociatedTokenAddressSync(
      usdcMint,
      distAddr,
      true
    );

    it("Create Pool", async () => {
      await program.methods
        .createPool(
          payer.publicKey, // authority
          new anchor.BN(100_000e6), // seed
          new anchor.BN(100), // shares
          new anchor.BN(3_000e6) // deposit
        )
        .accounts({
          creator: payer.publicKey,
          reference: reference.publicKey,
          pool: poolAddr,
          mint: mintAddr,
          creatorMintAccount,
          usdcMint,
          creatorUsdcAccount,
          poolUsdcAccount,
          metadata: metadataAddr,
          metadataProgram,
        })
        .signers([payer.payer, reference])
        .rpc();

      const poolAccount = await program.account.pool.fetch(poolAddr);
      expect(poolAccount.minted.eq(new anchor.BN(3))).toBe(true);
    });

    it("Close Pool", async () => {
      // Note: init pool distribution account before closing the pool
      await provider.connection.confirmTransaction(
        await program.methods
          .distribute(
            new anchor.BN(0) // 0 USDC
          )
          .accounts({
            authority: payer.publicKey,
            distributionAuthority: distAuthority.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            usdcMint,
            distributionUsdcAccount,
          })
          .signers([payer.payer, distAuthority])
          .rpc(),
        "confirmed"
      );

      await program.methods
        .closePool()
        .accounts({
          authority: payer.publicKey,
          pool: poolAddr,
          distribution: distAddr,
          usdcMint,
          poolUsdcAccount,
          distributionUsdcAccount,
        })
        .signers([payer.payer])
        .rpc();

      const poolAccount = await program.account.pool.fetch(poolAddr);
      expect(poolAccount.closed).toBe(true);
    });

    it("Should fail when attempting to re-close the pool", async () => {
      expect(
        program.methods
          .closePool()
          .accounts({
            authority: payer.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            usdcMint,
            poolUsdcAccount,
            distributionUsdcAccount,
          })
          .signers([payer.payer])
          .rpc()
      ).rejects.toThrow("Pool closed");
    });

    it("Should fail when attempting to close the pool with available holders' balance", async () => {
      expect(
        program.methods
          .closePoolAccounts()
          .accounts({
            authority: payer.publicKey,
            pool: poolAddr,
            distribution: distAddr,
            mint: mintAddr,
            usdcMint,
            poolUsdcAccount,
            distributionUsdcAccount,
          })
          .signers([payer.payer])
          .rpc()
      ).rejects.toThrow("Non-zero pool USDC balance");
    });

    it("Claim deposited USDC", async () => {
      const holderMintAccount = splToken.getAssociatedTokenAddressSync(
        mintAddr,
        payer.publicKey
      );
      const holderUsdcAccount = splToken.getAssociatedTokenAddressSync(
        usdcMint,
        payer.publicKey
      );

      await program.methods
        .claimDeposit()
        .accounts({
          holder: payer.publicKey,
          pool: poolAddr,
          mint: mintAddr,
          holderMintAccount,
          usdcMint,
          poolUsdcAccount,
          holderUsdcAccount,
        })
        .signers([payer.payer])
        .rpc();

      const balance = await provider.connection.getTokenAccountBalance(
        holderMintAccount
      );
      expect(balance.value.uiAmount).toBe(0);
    });

    it("Close Pool Accounts", async () => {
      await program.methods
        .closePoolAccounts()
        .accounts({
          authority: payer.publicKey,
          pool: poolAddr,
          distribution: distAddr,
          mint: mintAddr,
          usdcMint,
          poolUsdcAccount,
          distributionUsdcAccount,
        })
        .signers([payer.payer])
        .rpc();

      const poolAccount = await program.account.pool.fetchNullable(poolAddr);
      expect(poolAccount).toBe(null);
    });
  });
});
