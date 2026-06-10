#!/bin/sh
# Runs INSIDE a disposable container (root): installs prerequisites with the
# distro's package manager, then exercises sync-ai-clis for real — install
# via --yes, re-verify each tool's --version, and show the list view.
# POSIX sh on purpose: alpine/busybox has no bash until prep installs it.
set -eu

TOOLS="${TOOLS:-claude,antigravity}"

echo "--- prep: curl, ca-certificates, bash"
if command -v apt-get >/dev/null 2>&1; then
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y -qq curl ca-certificates bash >/dev/null
elif command -v dnf >/dev/null 2>&1; then
  # --allowerasing: rocky/alma ship curl-minimal which conflicts with curl
  dnf install -y --allowerasing curl ca-certificates bash >/dev/null
elif command -v pacman >/dev/null 2>&1; then
  pacman -Sy --noconfirm curl ca-certificates bash >/dev/null
elif command -v apk >/dev/null 2>&1; then
  apk add --no-cache curl ca-certificates bash libgcc >/dev/null
else
  echo "no known package manager in this image" >&2
  exit 2
fi

# Native installers drop binaries into ~/.local/bin; expose it up front so
# the engine's post-install verification sees them immediately.
PATH="$HOME/.local/bin:$PATH"
export PATH

echo "--- sync-ai-clis --yes --only $TOOLS"
sync-ai-clis --yes --only "$TOOLS"

for id in $(printf '%s' "$TOOLS" | tr ',' ' '); do
  case "$id" in
    claude) bin=claude ;;
    codex) bin=codex ;;
    gemini) bin=gemini ;;
    kiro) bin=kiro-cli ;;
    antigravity) bin=agy ;;
    *)
      echo "unknown tool id: $id" >&2
      exit 2
      ;;
  esac
  echo "--- verify: $bin --version"
  "$bin" --version
done

echo "--- second run: update path on freshly installed tools"
sync-ai-clis --yes --only "$TOOLS"

echo "--- sync-ai-clis list"
sync-ai-clis list
