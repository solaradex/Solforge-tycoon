# SolForge deployment

## GitHub Actions controls

Deploy SolForge Web runs on every push to main.
Build SolForge Tycoon Mobile runs on every push to main and can also be started with Run workflow.
Deploy SolForge Mainnet is intentionally manual-only because it changes real Solana mainnet state.

## Mainnet bootstrap requirements

The mainnet workflow requires these GitHub Actions secrets:

- SOLANA_DEPLOYER_KEYPAIR_B64 — base64-encoded JSON keypair for the wallet that should own the game configuration and pay deployment costs.
- SOLFORGE_PROGRAM_KEYPAIR_B64 — a separate base64-encoded Solana keypair for the SolForge game program. This keypair determines the program ID.
- SOLANA_RPC_URL — optional. Defaults to the public Solana Mainnet RPC endpoint.

Do not commit either private key or paste a private key into source code or chat.

## Mainnet sequence

The workflow performs:

1. Build the Anchor program.
2. Deploy the program.
3. Create the HASH SPL mint with 0 decimals.
4. Initialize the on-chain game configuration with a 1,000,000,000 HASH cap.
5. Transfer HASH mint authority to the game config PDA.
6. Publish the public program and mint addresses to deployments/solforge-mainnet.json.

The current token controller uses Solana's original Token Program for broad wallet compatibility. Solana also supports Token-2022; mint extensions generally must be planned at mint creation because most cannot be added after initialization.

## Important launch boundary

A wallet address alone cannot sign a Solana deployment. The actual mainnet bootstrap requires control of the wallet keypair. The repository is prepared for that signer-controlled step; the keypair is deliberately not stored in Git.

After the deployment workflow completes, its public deployments/solforge-mainnet.json record is committed back to the repository. The web client reads that file to enable the on-chain player-state bridge.
