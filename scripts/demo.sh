#!/usr/bin/env bash
# Start the sidecar, capture the Unix-socket URI, run the Go example, then stop.
# Requires: Rust 1.87+, Go 1.22+, Linux or macOS.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

if [[ "$(uname -s)" == "MINGW"* || "$(uname -s)" == "CYGWIN"* ]]; then
  echo "pglite-sidecar is Unix-only (Linux/macOS)." >&2
  exit 1
fi

echo "building pglite-sidecar…" >&2
cargo build --quiet

sidecar="$root/target/debug/pglite-sidecar"
data_dir="${PGLITE_DATA_DIR:-$root/data}"
mkdir -p "$data_dir"

uri_file="$(mktemp)"
err_file="$(mktemp)"
cleanup() {
  if [[ -n "${sidecar_pid:-}" ]] && kill -0 "$sidecar_pid" 2>/dev/null; then
    kill -TERM "$sidecar_pid" 2>/dev/null || true
    wait "$sidecar_pid" 2>/dev/null || true
  fi
  rm -f "$uri_file" "$err_file"
}
trap cleanup EXIT

echo "starting sidecar (data dir: $data_dir)…" >&2
"$sidecar" --data-dir "$data_dir" --extra-connections "${PGLITE_EXTRA_CONNECTIONS:-4}" \
  >"$uri_file" 2>"$err_file" &
sidecar_pid=$!

uri=""
for _ in $(seq 1 120); do
  if ! kill -0 "$sidecar_pid" 2>/dev/null; then
    echo "sidecar exited before printing a URI:" >&2
    cat "$err_file" >&2
    exit 1
  fi
  if [[ -s "$uri_file" ]]; then
    uri="$(head -n 1 "$uri_file")"
    if [[ "$uri" == postgresql://* ]]; then
      break
    fi
  fi
  sleep 0.25
done

if [[ "$uri" != postgresql://* ]]; then
  echo "timed out waiting for the sidecar URI. sidecar stderr:" >&2
  cat "$err_file" >&2
  exit 1
fi

echo "DATABASE_URL=$uri" >&2
export DATABASE_URL="$uri"

(cd "$root/examples/go" && go run .)
