import * as anchor from "@coral-xyz/anchor";
import { Program, web3 } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  SYSVAR_RENT_PUBKEY
} from "@solana/web3.js";
import {
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  MINT_SIZE,
  createInitializeMint2Instruction,
  createAssociatedTokenAccountInstruction,
  createMintToCheckedInstruction
} from "@solana/spl-token";
import { BN } from "@coral-xyz/anchor";
import { TokenEscrowAnchor } from "../target/types/token_escrow_anchor";

describe("token-escrow-anchor", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace
    .TokenEscrowAnchor as Program<TokenEscrowAnchor>;

  const payer = program.provider.publicKey;

  const authorityKp = new Keypair();
  const authority = authorityKp.publicKey;

  const takerKp = new Keypair();
  const taker = takerKp.publicKey;

  const sellMintKp = new Keypair();
  const buyMintKp = new Keypair();

  const sellMint = sellMintKp.publicKey;
  const buyMint = buyMintKp.publicKey;

  const authoritySellAta = getAssociatedTokenAddressSync(sellMint, authority);
  const authorityBuyAta = getAssociatedTokenAddressSync(buyMint, authority);

  const takerSellAta = getAssociatedTokenAddressSync(buyMint, taker);
  const takerBuyAta = getAssociatedTokenAddressSync(sellMint, taker);

  const escrow = PublicKey.findProgramAddressSync(
    [Buffer.from("escrow"), authority.toBuffer(), sellMint.toBuffer()],
    program.programId
  )[0];

  const escrowAta = getAssociatedTokenAddressSync(sellMint, escrow, true);

  const rent = SYSVAR_RENT_PUBKEY;
  const tokenProgram = TOKEN_PROGRAM_ID;
  const systemProgram = SystemProgram.programId;
  const associatedTokenProgram = ASSOCIATED_TOKEN_PROGRAM_ID;

  const sellTokenDecimals = Math.floor(Math.random() * 10);
  const buyTokenDecimals = Math.floor(Math.random() * 10);

  const sellAmount = new BN(Math.floor(Math.random() * 1000000000));
  const buyAmount = new BN(Math.floor(Math.random() * 1000000000));

  console.log({
    payer: payer.toBase58(),
    authority: authority.toBase58(),
    taker: taker.toBase58(),
    sellMint: sellMint.toBase58(),
    buyMint: buyMint.toBase58(),
    authoritySellAta: authoritySellAta.toBase58(),
    authorityBuyAta: authorityBuyAta.toBase58(),
    takerSellAta: takerSellAta.toBase58(),
    takerBuyAta: takerBuyAta.toBase58(),
    escrow: escrow.toBase58(),
    escrowAta: escrowAta.toBase58(),
    rent: rent.toBase58(),
    tokenProgram: tokenProgram.toBase58(),
    systemProgram: systemProgram.toBase58(),
    associatedTokenProgram: associatedTokenProgram.toBase58(),
    sellTokenDecimals: sellTokenDecimals,
    buyTokenDecimals: buyTokenDecimals,
    sellAmount: sellAmount.toString(),
    buyAmount: buyAmount.toString()
  });

  it("Initialize Escrow", async () => {
    let mint_lamports =
      await program.provider.connection.getMinimumBalanceForRentExemption(
        MINT_SIZE
      );

    let sellMintCreateIx = SystemProgram.createAccount({
      fromPubkey: payer,
      lamports: mint_lamports,
      newAccountPubkey: sellMint,
      programId: tokenProgram,
      space: MINT_SIZE
    });

    let buyMintCreateIx = SystemProgram.createAccount({
      fromPubkey: payer,
      lamports: mint_lamports,
      newAccountPubkey: buyMint,
      programId: tokenProgram,
      space: MINT_SIZE
    });
    let sellMintInitialize = createInitializeMint2Instruction(
      sellMint,
      sellTokenDecimals,
      payer,
      payer
    );
    let buyMintInitialize = createInitializeMint2Instruction(
      buyMint,
      buyTokenDecimals,
      payer,
      payer
    );

    let authoritySellTokenAccountCreateIx =
      createAssociatedTokenAccountInstruction(
        payer,
        authoritySellAta,
        authority,
        sellMint
      );

    let authorityBuyTokenAccountCreateIx =
      createAssociatedTokenAccountInstruction(
        payer,
        authorityBuyAta,
        authority,
        buyMint
      );

    let takerBuyTokenAccountCreateIx = createAssociatedTokenAccountInstruction(
      payer,
      takerBuyAta,
      taker,
      sellMint
    );

    let takerSellTokenAccountCreateIx = createAssociatedTokenAccountInstruction(
      payer,
      takerSellAta,
      taker,
      buyMint
    );

    let transferIx1 = SystemProgram.transfer({
      fromPubkey: payer,
      toPubkey: authority,
      lamports: 10000000000
    });
    let transferIx2 = SystemProgram.transfer({
      fromPubkey: payer,
      toPubkey: taker,
      lamports: 10000000000
    });

    let mintToIx1 = createMintToCheckedInstruction(
      sellMint,
      authoritySellAta,
      payer,
      BigInt(Math.floor(Math.random() * 10000000000)),
      sellTokenDecimals
    );
    let mintToIx2 = createMintToCheckedInstruction(
      buyMint,
      takerSellAta,
      payer,
      BigInt(Math.floor(Math.random() * 10000000000)),
      buyTokenDecimals
    );
    // Add your test here.
    const tx = await program.methods
      .initEscrow(sellAmount, buyAmount)
      .accounts({
        sellMint,
        buyMint,
        authority,
        authoritySellAta,
        authorityBuyAta,
        escrow,
        escrowAta,
        rent,
        systemProgram,
        tokenProgram,
        associatedTokenProgram
      })
      .preInstructions([
        sellMintCreateIx,
        buyMintCreateIx,
        sellMintInitialize,
        buyMintInitialize,
        authoritySellTokenAccountCreateIx,
        authorityBuyTokenAccountCreateIx,
        takerBuyTokenAccountCreateIx,
        takerSellTokenAccountCreateIx,
        mintToIx1,
        mintToIx2,
        transferIx1,
        transferIx2
      ])
      .signers([authorityKp, sellMintKp, buyMintKp])
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);
  });

  it("Exchange", async () => {
    // Add your test here.
    const tx = await program.methods
      .exchange(buyAmount, sellAmount)
      .accounts({
        authority,
        taker,
        takerSellMint: buyMint,
        takerBuyMint: sellMint,
        takerSellAta,
        takerBuyAta,
        receiveAta: authorityBuyAta,
        escrow,
        escrowAta,
        tokenProgram,
        associatedTokenProgram,
        rent,
        systemProgram
      })
      .signers([takerKp])
      .rpc({ skipPreflight: true });
    console.log("Your transaction signature", tx);
  });
});
