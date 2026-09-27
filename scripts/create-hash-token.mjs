import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {
  Connection,
  Keypair,
  PublicKey
} from '@solana/web3.js';
import {
  createMint,
  TOKEN_PROGRAM_ID
} from '@solana/spl-token';

const rpc = process.env.SOLANA_RPC_URL || 'https://api.mainnet-beta.solana.com';
const keypairPath = (process.env.SOLANA_KEYPAIR || '~/.config/solana/id.json')
  .replace(/^~(?=\/|\\)/, os.homedir());

if (!fs.existsSync(keypairPath)) {
  throw new Error(`Signer keypair not found: ${keypairPath}`);
}

const secret = JSON.parse(fs.readFileSync(keypairPath, 'utf8'));
const payer = Keypair.fromSecretKey(Uint8Array.from(secret));
const connection = new Connection(rpc, 'confirmed');

const mint = await createMint(
  connection,
  payer,
  payer.publicKey,
  null,
  0,
  Keypair.generate(),
  { commitment: 'confirmed' },
  TOKEN_PROGRAM_ID
);

const output = {
  name: 'SolForge Hash',
  symbol: 'HASH',
  decimals: 0,
  mint: mint.toBase58(),
  tokenProgram: TOKEN_PROGRAM_ID.toBase58(),
  initialMintAuthority: payer.publicKey.toBase58(),
  rpc
};

fs.mkdirSync('deployments', { recursive: true });
fs.writeFileSync('deployments/hash-token.json', JSON.stringify(output, null, 2) + '\n');

console.log(JSON.stringify(output, null, 2));
console.log('Next: deploy the SolForge game program, initialize_game with this mint, then run set:hash-mint-authority.');
