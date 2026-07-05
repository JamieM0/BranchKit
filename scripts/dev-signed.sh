#!/usr/bin/env bash
# Dev-mode launcher that codesigns the debug binary with a stable local identity before
# running it, so macOS Keychain's "Always Allow" grant survives across rebuilds instead of
# reprompting on every launch. See scripts/dev-cert-setup.sh (run that once first) for why.
#
# Equivalent to `npm run tauri dev`, except it drives cargo/vite itself so it can codesign the
# binary in between building and launching it (the tauri-cli `tauri dev` command runs the raw
# binary straight after `cargo build` with no hook in between).

set -euo pipefail

CERT_NAME="BranchKit Dev Signing"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="$ROOT_DIR/src-tauri/target/debug/branchkit"

if ! security find-identity -v -p codesigning 2>/dev/null | grep -q "$CERT_NAME"; then
  echo "No '$CERT_NAME' signing identity found." >&2
  echo "Run 'bash scripts/dev-cert-setup.sh' once, then try again." >&2
  exit 1
fi

VITE_PID=""
cleanup() {
  if [[ -n "$VITE_PID" ]]; then
    kill "$VITE_PID" 2>/dev/null || true
    wait "$VITE_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

echo "Starting Vite dev server on :1420..."
(cd "$ROOT_DIR" && npm run dev) &
VITE_PID=$!

echo "Waiting for http://localhost:1420 ..."
for _ in $(seq 1 120); do
  if curl -sf http://localhost:1420 >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

echo "Building (cargo build)..."
(cd "$ROOT_DIR/src-tauri" && cargo build)

echo "Codesigning $BIN_PATH with '$CERT_NAME'..."
codesign --force --sign "$CERT_NAME" "$BIN_PATH"

echo "Launching BranchKit..."
"$BIN_PATH"
