#!/usr/bin/env bash
set -euo pipefail
umask 077

REPO="solaradex/Solforge-tycoon"
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

DEPLOYER="$HOME/.config/solana/solforge-deployer.json"
PROGRAM="$HOME/.config/solana/solforge-game-program.json"
mkdir -p "$(dirname "$DEPLOYER")"

command -v solana-keygen >/dev/null || {
  echo "Missing solana-keygen."
  echo "Install the Solana CLI first, then rerun this script."
  exit 1
}
command -v gh >/dev/null || {
  echo "Missing GitHub CLI (gh). Install it, then rerun this script."
  exit 1
}
gh auth status >/dev/null 2>&1 || {
  echo "GitHub CLI is not authenticated."
  echo "Run: gh auth login --scopes repo,workflow"
  exit 1
}

echo "=== 1. Create deployer keypair ==="
if [[ ! -f "$DEPLOYER" ]]; then
  solana-keygen new --outfile "$DEPLOYER" --no-bip39-passphrase
else
  echo "Using existing: $DEPLOYER"
fi

echo "=== 2. Create program keypair ==="
if [[ ! -f "$PROGRAM" ]]; then
  solana-keygen new --outfile "$PROGRAM" --no-bip39-passphrase
else
  echo "Using existing: $PROGRAM"
fi

DEPLOYER_PUB="$(solana-keygen pubkey "$DEPLOYER")"
PROGRAM_PUB="$(solana-keygen pubkey "$PROGRAM")"

echo
echo "Deployer public key: $DEPLOYER_PUB"
echo "Program public key:  $PROGRAM_PUB"
echo
echo "IMPORTANT: the public keys above are safe to share; never share the JSON keypair files."

echo "=== 3. Update Anchor program ID ==="
python3 - "$PROGRAM_PUB" <<'PY'
from pathlib import Path
import re
import sys

program_id = sys.argv[1]

anchor = Path("Anchor.toml")
text = anchor.read_text()
text = re.sub(
    r'([programs.mainnet]s*solforge_games*=s*")[^"]+(")',
    rf'\g<1>{program_id}\g<2>',
    text
)
text = re.sub(
    r'([programs.devnet]s*solforge_games*=s*")[^"]+(")',
    rf'\g<1>{program_id}\g<2>',
    text
)
anchor.write_text(text)

lib = Path("programs/solforge-game/src/lib.rs")
text = lib.read_text()
text, n = re.subn(
    r'declare_id!("[1-9A-HJ-NP-Za-km-z]{32,44}");',
    f'declare_id!("{program_id}");',
    text,
    count=1
)
if n != 1:
    raise SystemExit("Could not update declare_id! in lib.rs")
lib.write_text(text)
PY

echo "Anchor.toml and lib.rs now use: $PROGRAM_PUB"

echo "=== 4. Commit public program-ID changes ==="
git add Anchor.toml programs/solforge-game/src/lib.rs
if ! git diff --cached --quiet; then
  git config user.name >/dev/null 2>&1 || git config user.name "solaradex"
  git config user.email >/dev/null 2>&1 || git config user.email "solaradex@users.noreply.github.com"
  git commit -m "Set SolForge program ID from generated keypair"
  git push origin main
else
  echo "No public program-ID changes to commit."
fi

echo "=== 5. Upload encrypted GitHub Actions secrets ==="
base64 -w 0 "$DEPLOYER" | gh secret set SOLANA_DEPLOYER_KEYPAIR_B64 --repo "$REPO"
base64 -w 0 "$PROGRAM" | gh secret set SOLFORGE_PROGRAM_KEYPAIR_B64 --repo "$REPO"

echo "=== 6. Configure mainnet RPC ==="
read -r -p "Mainnet RPC URL (press Enter for public Solana RPC): " RPC
RPC="${RPC:-https://api.mainnet-beta.solana.com}"
printf '%s' "$RPC" | gh secret set SOLANA_RPC_URL --repo "$REPO"

echo
echo "=== 7. Verify secret names (values are never printed) ==="
gh secret list --repo "$REPO" | grep -E 'SOLANA_DEPLOYER_KEYPAIR_B64|SOLFORGE_PROGRAM_KEYPAIR_B64|SOLANA_RPC_URL' || true

echo
echo "=== 8. Optional local Solana CLI setup ==="
solana config set --url mainnet-beta --keypair "$DEPLOYER" >/dev/null
solana address
solana balance

echo
echo "DONE."
echo "Program ID: $PROGRAM_PUB"
echo "Deployer:   $DEPLOYER_PUB"
echo
echo "Next:"
echo "  gh workflow run 'Deploy SolForge Mainnet' --repo $REPO --ref main"
echo "  gh run list --repo $REPO --limit 10"
