#!/usr/bin/env bash
#
# Clone (or fast-forward) VexFS into vendor/vexfs at the pinned commit,
# then apply two small patches that make the upstream Dockerfile build
# cleanly. Run this once before `docker compose up` on a fresh machine.
#
# Upstream bugs at the pinned SHA (both present on main at time of pin):
#
#   1. The git tree contains a gitlink at `tests/xfstests/xfstests-dev`
#      but no `.gitmodules` entry describing it, so any recursive
#      submodule init (including Docker Compose's git-URL build context)
#      fails with "fatal: No url found for submodule path ...".
#      → Cloning with --no-recurse-submodules sidesteps this.
#
#   2. `Dockerfile` has `COPY docker-entrypoint.sh /usr/local/bin/`, but
#      the script actually lives at `docker/docker-entrypoint.sh`. The
#      intended entrypoint (the longer one, with dashboard-on-single-port
#      support) is docker/docker-entrypoint.sh — copying it to the repo
#      root makes the COPY line resolve.
#      → `cp docker/docker-entrypoint.sh ./docker-entrypoint.sh` after clone.
#
# Both should eventually be fixed upstream (this is github.com/lspecian/vexfs,
# maintained by the Specian family — the same team shipping Historiador).
# When they are, drop the cp line and bump the pinned SHA.
#
# The pinned SHA is intentionally out-of-band from docker-compose.yml —
# we want both files to reference the same commit. Keep them in sync
# when bumping.

set -euo pipefail

VEXFS_REPO_URL="https://github.com/lspecian/vexfs.git"
VEXFS_PINNED_SHA="5a0609dd0984afafef652c4271c6de69aec5c9be"
VENDOR_DIR="vendor/vexfs"

cd "$(dirname "$0")/.."

if [[ -d "$VENDOR_DIR/.git" ]]; then
    echo "→ vendor/vexfs exists, fetching…"
    git -C "$VENDOR_DIR" fetch --depth 1 origin "$VEXFS_PINNED_SHA"
    git -C "$VENDOR_DIR" checkout --detach "$VEXFS_PINNED_SHA"
else
    echo "→ cloning vexfs into $VENDOR_DIR at $VEXFS_PINNED_SHA"
    mkdir -p "$(dirname "$VENDOR_DIR")"
    git clone --no-recurse-submodules "$VEXFS_REPO_URL" "$VENDOR_DIR"
    git -C "$VENDOR_DIR" checkout --detach "$VEXFS_PINNED_SHA"
fi

# --- Patch 2: copy the intended docker-entrypoint.sh to the repo root ---
# Upstream's Dockerfile does `COPY docker-entrypoint.sh /usr/local/bin/` but
# the script actually lives at `docker/docker-entrypoint.sh`. Copy it up.
if [[ -f "$VENDOR_DIR/docker/docker-entrypoint.sh" && ! -f "$VENDOR_DIR/docker-entrypoint.sh" ]]; then
    cp "$VENDOR_DIR/docker/docker-entrypoint.sh" "$VENDOR_DIR/docker-entrypoint.sh"
    chmod +x "$VENDOR_DIR/docker-entrypoint.sh"
    echo "→ patched vendor/vexfs/docker-entrypoint.sh (copied from docker/)"
fi

echo "✓ vendor/vexfs is at $(git -C "$VENDOR_DIR" rev-parse --short HEAD)"
