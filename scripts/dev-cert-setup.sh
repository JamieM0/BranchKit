#!/usr/bin/env bash
# One-time setup: creates a self-signed code-signing certificate with a stable name/identity
# and imports it into the login keychain.
#
# Why this exists: `tauri dev` runs the raw, unsigned debug binary directly (it doesn't go
# through the bundler/codesign step that `tauri build` uses, so tauri.conf.json's
# bundle.macOS.signingIdentity doesn't apply here). Without ANY stable signature, macOS
# Keychain ties "Always Allow" grants to the binary's identity — and an unsigned/ad-hoc-signed
# binary gets a new identity on every rebuild, so the GitHub token / AI API key keychain
# prompts (credentials.rs, service "BranchKit") reappear on every launch even after granting
# access. Signing the dev binary with the same certificate every time gives it a stable
# identity so macOS actually remembers the grant. See ARCHITECTURE.md §14 (codesign/notarize
# for real distribution is still deferred separately — this is a dev-only convenience).
#
# Run once: bash scripts/dev-cert-setup.sh
# Then use: npm run tauri:dev:signed   (instead of `npm run tauri dev`)

set -euo pipefail

CERT_NAME="BranchKit Dev Signing"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning "$KEYCHAIN" 2>/dev/null | grep -q "$CERT_NAME"; then
  echo "Signing identity '$CERT_NAME' already exists in the login keychain — nothing to do."
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/cert.conf" <<EOF
[req]
distinguished_name = req_distinguished_name
x509_extensions = v3_req
prompt = no

[req_distinguished_name]
CN = $CERT_NAME

[v3_req]
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, codeSigning
basicConstraints = critical, CA:false
EOF

echo "Generating self-signed certificate '$CERT_NAME'..."
openssl req -x509 -newkey rsa:2048 -keyout "$TMP/key.pem" -out "$TMP/cert.pem" \
  -days 3650 -nodes -config "$TMP/cert.conf" -extensions v3_req

# -legacy: OpenSSL 3.x defaults to a PKCS12 encryption scheme macOS's `security import` can't
# read, which fails as "MAC verification failed" (misleadingly looks like a wrong password).
# Older OpenSSL (1.1.x, e.g. LibreSSL on some Macs) doesn't recognize -legacy, so fall back.
if openssl pkcs12 -help 2>&1 | grep -q -- -legacy; then
  openssl pkcs12 -export -legacy -out "$TMP/cert.p12" \
    -inkey "$TMP/key.pem" -in "$TMP/cert.pem" -passout pass:branchkit-dev
else
  openssl pkcs12 -export -out "$TMP/cert.p12" \
    -inkey "$TMP/key.pem" -in "$TMP/cert.pem" -passout pass:branchkit-dev
fi

echo "Importing into the login keychain (you'll be asked for your macOS password)..."
security import "$TMP/cert.p12" -k "$KEYCHAIN" -P branchkit-dev \
  -T /usr/bin/codesign -T /usr/bin/security

echo "Trusting the certificate for code signing..."
security add-trusted-cert -d -r trustAsRoot -p codeSign -k "$KEYCHAIN" "$TMP/cert.pem" || {
  echo
  echo "Automatic trust step failed or needs confirmation. Finish it manually:"
  echo "  1. Open Keychain Access -> login keychain -> My Certificates"
  echo "  2. Double-click '$CERT_NAME'"
  echo "  3. Expand Trust, set 'Code Signing' to 'Always Trust', close the panel"
  echo "  4. Enter your password when prompted"
}

echo
echo "Done. Run 'npm run tauri:dev:signed' for a dev build that keeps a stable identity"
echo "across rebuilds, so the Keychain 'Always Allow' grant actually sticks."
