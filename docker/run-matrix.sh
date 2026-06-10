#!/usr/bin/env bash
# Layer-2 Linux integration matrix (SPEC §8.2): build the linux binary inside
# a rust container, then run REAL installs/updates in disposable distro
# containers. This is the only place real execution is allowed locally
# (SPEC §8.5) — the host is never touched and no ports are published.
#
# Usage:
#   docker/run-matrix.sh                     # smoke: ubuntu:24.04, claude+antigravity
#   MATRIX=full docker/run-matrix.sh         # all glibc distros
#   TOOLS=claude,codex,kiro,antigravity docker/run-matrix.sh
#   RUN_MUSL=1 docker/run-matrix.sh          # extra: musl build + alpine leg (experimental)
#   PLATFORM=linux/amd64 docker/run-matrix.sh  # cross-arch via QEMU
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="${TOOLS:-claude,antigravity}"
MATRIX="${MATRIX:-smoke}"
PLATFORM="${PLATFORM:-}"

case "$MATRIX" in
  smoke) DISTROS=(ubuntu:24.04) ;;
  full) DISTROS=(ubuntu:22.04 ubuntu:24.04 debian:12 fedora:latest archlinux rockylinux:9) ;;
  *)
    echo "MATRIX must be smoke|full (got: $MATRIX)" >&2
    exit 2
    ;;
esac

PLATFORM_ARGS=()
[[ -n "$PLATFORM" ]] && PLATFORM_ARGS=(--platform "$PLATFORM")

echo "==> Building linux (glibc) release binary in a rust container"
docker run --rm "${PLATFORM_ARGS[@]}" \
  -v "$ROOT":/src -w /src \
  -v sync-ai-clis-cargo-registry:/usr/local/cargo/registry \
  rust:1-bookworm \
  cargo build --release --quiet --target-dir /src/target-linux
BIN="$ROOT/target-linux/release/sync-ai-clis"

run_leg() {
  local image="$1" binary="$2"
  echo
  echo "==> [$image] real install/update of: $TOOLS"
  docker run --rm "${PLATFORM_ARGS[@]}" \
    -v "$binary":/usr/local/bin/sync-ai-clis:ro \
    -v "$ROOT/docker/container-test.sh":/container-test.sh:ro \
    -e TOOLS="$TOOLS" \
    "$image" sh /container-test.sh
}

FAILED=()
for distro in "${DISTROS[@]}"; do
  if run_leg "$distro" "$BIN"; then
    echo "==> [$distro] OK"
  else
    echo "==> [$distro] FAIL"
    FAILED+=("$distro")
  fi
done

if [[ "${RUN_MUSL:-0}" == "1" ]]; then
  echo
  echo "==> Building linux (musl) release binary in a rust:alpine container"
  docker run --rm "${PLATFORM_ARGS[@]}" \
    -v "$ROOT":/src -w /src \
    -v sync-ai-clis-cargo-registry-musl:/usr/local/cargo/registry \
    rust:1-alpine \
    sh -c "apk add --no-cache musl-dev >/dev/null && cargo build --release --quiet --target-dir /src/target-linux-musl"
  MUSL_BIN="$ROOT/target-linux-musl/release/sync-ai-clis"
  if run_leg "alpine:3.20" "$MUSL_BIN"; then
    echo "==> [alpine:3.20/musl] OK"
  else
    echo "==> [alpine:3.20/musl] FAIL (experimental: upstream installers may not ship musl builds)"
    FAILED+=("alpine:3.20")
  fi
fi

echo
if ((${#FAILED[@]})); then
  echo "Matrix finished with failures: ${FAILED[*]}"
  exit 1
fi
echo "Matrix OK (${DISTROS[*]}${RUN_MUSL:+ + alpine}) — tools: $TOOLS"
