import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import {
  setAuthority,
  AuthorityType,
  TOKEN_PROGRAM_ID
} from '@solana/spl-token';

const rpc = process.env.SOLANA_RPC_URL || 'https://api.mainnet-beta.solana.com';
const programId = new PublicKey(process.env.SOLFORGE_PROGRAM_ID || 'F4rUAeBfjpeV5SLQJv6BCNhSr4QrignBDxFEzTU7bPyp');
const keypairPath = (process.env.SOLANA_KEYPAIR || '~/.config/solana/id.json')
  .replace(/^~(?=\/|\\)/, os.homedir());

const deploymentPath = path.resolve('deployments/hash-token.json');
if (!fs.existsSync(deploymentPath)) {
  throw new Error('Missing deployments/hash-token.json. Create the HASH mint first.');
}

if (!fs.existsSync(keypairPath)) {
  throw new Error(`Signer keypair not found: ${keypairPath}`);
}

const token = JSON.parse(fs.readFileSync(deploymentPath, 'utf8'));
const mint = new PublicKey(token.mint);
const secret = JSON.parse(fs.readFileSync(keypairPath, 'utf8'));
const payer = Keypair.fromSecretKey(Uint8Array.from(secret));
const connection = new Connection(rpc, 'confirmed');

const [configPda] = PublicKey.findProgramAddressSync([Buffer.from('config')], programId);

if (token.initialMintAuthority !== payer.publicKey.toBase58()) {
  throw new Error('The configured signer is not the current mint authority.');
}

const signature = await setAuthority(
  connection,
  payer,
  mint,
  payer,
  AuthorityType.MintTokens,
  configPda,
  [],
  { commitment: 'confirmed' },
  TOKEN_PROGRAM_ID
);

const updated = {
  ...token,
  mintAuthority: configPda.toBase58(),
  gameProgram: programId.toBase58(),
  authorityTransferSignature: signature
};

fs.writeFileSync(deploymentPath, JSON.stringify(updated, null, 2) + '\n');
console.log(JSON.stringify(updated, null, 2));
