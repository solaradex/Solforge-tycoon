import fs from 'node:fs';
import os from 'node:os';
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, TransactionInstruction } from '@solana/web3.js';

const rpc = process.env.SOLANA_RPC_URL || 'https://api.mainnet-beta.solana.com';
const programId = new PublicKey(process.env.SOLFORGE_PROGRAM_ID || 'F4rUAeBfjpeV5SLQJv6BCNhSr4QrignBDxFEzTU7bPyp');
const keypairPath = (process.env.SOLANA_KEYPAIR || '~/.config/solana/id.json')
  .replace(/^~(?=\/|\\)/, os.homedir());
const tokenPath = 'deployments/hash-token.json';

if (!fs.existsSync(keypairPath)) throw new Error(`Signer keypair not found: ${keypairPath}`);
if (!fs.existsSync(tokenPath)) throw new Error('Missing deployments/hash-token.json');

const secret = JSON.parse(fs.readFileSync(keypairPath, 'utf8'));
const authority = Keypair.fromSecretKey(Uint8Array.from(secret));
const token = JSON.parse(fs.readFileSync(tokenPath, 'utf8'));
const hashMint = new PublicKey(token.mint);
const connection = new Connection(rpc, 'confirmed');

const [configPda, bump] = PublicKey.findProgramAddressSync([Buffer.from('config')], programId);
const existing = await connection.getAccountInfo(configPda);
if (existing) {
  console.log(`Game config already initialized: ${configPda.toBase58()}`);
  process.exit(0);
}

const discriminator = Buffer.from('2c3e66f77ed082d7', 'hex');
const data = Buffer.alloc(8 + 32 + 8);
discriminator.copy(data, 0);
hashMint.toBuffer().copy(data, 8);
data.writeBigUInt64LE(1000000000n, 40);

const ix = new TransactionInstruction({
  programId,
  keys: [
    { pubkey: configPda, isSigner: false, isWritable: true },
    { pubkey: authority.publicKey, isSigner: true, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false }
  ],
  data
});

const tx = new Transaction().add(ix);
tx.feePayer = authority.publicKey;
const latest = await connection.getLatestBlockhash('confirmed');
tx.recentBlockhash = latest.blockhash;

const signature = await (async () => {
  const signed = await authority.signTransaction(tx);
  return await connection.sendRawTransaction(signed.serialize(), { skipPreflight: false });
})();
await connection.confirmTransaction({
  signature,
  blockhash: latest.blockhash,
  lastValidBlockHeight: latest.lastValidBlockHeight
}, 'confirmed');

console.log(JSON.stringify({
  config: configPda.toBase58(),
  bump,
  hashMint: hashMint.toBase58(),
  signature
}, null, 2));
