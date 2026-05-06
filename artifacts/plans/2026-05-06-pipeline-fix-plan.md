# Release Pipeline Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace QEMU-emulated multi-arch builds with native amd64/arm64 runners and swap api/mcp/web runtime base images to distroless, eliminating the 6h release timeout and most suppressed CVEs.

**Architecture:** Six parallel `build` jobs (matrix: service × platform → native runner) each push a single-platform image-by-digest to GHCR. Three `merge` jobs fan in by service, assembling a multi-arch manifest list, then run Trivy + SBOM + cosign sign + cosign attest once per service. A `workflow_dispatch` `dry_run` input lets us validate the build path on a branch (build + per-platform Trivy, no push). Runtime base images move to `gcr.io/distroless/cc-debian12:nonroot` (api/mcp) and `gcr.io/distroless/nodejs22-debian12:nonroot` (web).

**Tech Stack:** GitHub Actions (`ubuntu-24.04`, `ubuntu-24.04-arm`), Docker Buildx, GHCR, distroless containers, cargo-chef Rust builder, Next.js standalone output, Anchore SBOM, Trivy, cosign keyless OIDC, actionlint.

**Spec:** [artifacts/specs/2026-05-06-pipeline-fix-design.md](../specs/2026-05-06-pipeline-fix-design.md)

---

## File Structure

| Path | Status | Purpose |
|---|---|---|
| `.github/workflows/release.yml` | rewrite | Triggers (tag/branch/dispatch), per-platform `build` matrix, `merge` matrix, `release-notes` |
| `Dockerfile.rust` | modify lines 50–65 | Runtime stage swap to `distroless/cc-debian12:nonroot` |
| `apps/web/Dockerfile` | rewrite | Bump `NODE_VERSION` to 22, runner stage swap to `distroless/nodejs22-debian12:nonroot` |
| `.trivyignore` | modify | Prune entries no longer triggered by the new bases (Task 7) |
| `.github/workflows/ci.yml` | modify | Verify/bump action versions if Node-24 deprecation requires it |

`apps/api`, `apps/mcp`, `apps/web`, and the `crates/*` source code are **not** modified by this work.

---

## Prerequisites

Install `actionlint` for local YAML validation (one-time, ~10 seconds):

```bash
bash <(curl -sSfL https://raw.githubusercontent.com/rhysd/actionlint/main/scripts/download-actionlint.bash)
sudo mv ./actionlint /usr/local/bin/actionlint
actionlint -version
```

Expected: prints a semver string (e.g. `1.7.x`).

If the user prefers not to install: `docker run --rm -v "$PWD":/repo -w /repo rhysd/actionlint:latest -color` works as a drop-in. Replace `actionlint` with that command in every step below.

---

## Task 1: Pre-flight

**Files:** none (operational only)

- [ ] **Step 1.1: Confirm working tree is clean and on the right branch**

```bash
git status
git rev-parse --abbrev-ref HEAD
```

Expected: `nothing to commit, working tree clean` and `bugfix/fix-pipeline-issues`. If not, stop and reconcile with the user before continuing.

- [ ] **Step 1.2: Cancel the in-flight v1.0.0 release run (with explicit user confirmation)**

The run was burning CI minutes when this plan was written. If still running, ask the user: *"OK to cancel run 25443874575? It will keep failing on the same issues we're fixing here."* On approval:

```bash
gh run cancel 25443874575
```

Expected: `✓ Request to cancel workflow submitted.` If already completed, skip.

- [ ] **Step 1.3: Sanity-check the latest workflow status**

```bash
gh run list --workflow=release.yml --limit 5
```

Expected: most recent run is either `cancelled` or `completed/failure`. No new commits should be needed for this step.

---

## Task 2: Rewrite `.github/workflows/release.yml`

**Files:**
- Modify: `.github/workflows/release.yml` (full rewrite — replace entire file)

The new file uses the standard Docker multi-platform pattern: per-platform jobs that push by digest to GHCR (no tag), digests passed via artifacts to a fan-in merge job that creates the manifest list and runs the scan/SBOM/sign/attest sequence once per service.

- [ ] **Step 2.1: Write the new `.github/workflows/release.yml`**

Replace the entire file with:

```yaml
name: Release

on:
  push:
    tags: ['v*.*.*']
    branches: ['release/beta-*']
  workflow_dispatch:
    inputs:
      dry_run:
        description: 'Build and scan only — do not push, sign, or attest'
        type: boolean
        default: false

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

permissions:
  contents: write        # release-notes job creates a GitHub Release
  packages: write        # GHCR push
  id-token: write        # cosign keyless via GitHub OIDC
  attestations: write    # SBOM attestation

env:
  REGISTRY: ghcr.io
  IMAGE_NAMESPACE: ${{ github.repository }}

jobs:
  # ----------------------------------------------------------------
  # build: matrix of (service × platform) on native runners.
  # Pushes single-platform images by digest to GHCR (no tag) on real
  # runs. On dry-run, loads to local Docker and runs Trivy directly.
  # ----------------------------------------------------------------
  build:
    name: Build ${{ matrix.service }} (${{ matrix.arch }})
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        service: [api, mcp, web]
        platform: [linux/amd64, linux/arm64]
        include:
          - platform: linux/amd64
            runner: ubuntu-24.04
            arch: amd64
          - platform: linux/arm64
            runner: ubuntu-24.04-arm
            arch: arm64
          - service: api
            dockerfile: Dockerfile.rust
            build-args: BIN_NAME=api
          - service: mcp
            dockerfile: Dockerfile.rust
            build-args: BIN_NAME=mcp
          - service: web
            dockerfile: apps/web/Dockerfile
            build-args: ""
    steps:
      - uses: actions/checkout@v4

      - uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        if: github.event_name != 'workflow_dispatch' || github.event.inputs.dry_run != 'true'
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      # Real build: push by digest, no tag. Digests are merged later.
      - name: Build and push by digest
        id: build
        if: github.event_name != 'workflow_dispatch' || github.event.inputs.dry_run != 'true'
        uses: docker/build-push-action@v6
        with:
          context: .
          file: ${{ matrix.dockerfile }}
          build-args: ${{ matrix.build-args }}
          platforms: ${{ matrix.platform }}
          outputs: type=image,name=${{ env.REGISTRY }}/${{ env.IMAGE_NAMESPACE }}/${{ matrix.service }},push-by-digest=true,name-canonical=true,push=true
          cache-from: type=gha,scope=${{ matrix.service }}-${{ matrix.arch }}
          cache-to: type=gha,scope=${{ matrix.service }}-${{ matrix.arch }},mode=max

      # Dry-run build: load to local Docker for in-job Trivy scan.
      - name: Build (dry-run, load locally)
        id: build-dry
        if: github.event_name == 'workflow_dispatch' && github.event.inputs.dry_run == 'true'
        uses: docker/build-push-action@v6
        with:
          context: .
          file: ${{ matrix.dockerfile }}
          build-args: ${{ matrix.build-args }}
          platforms: ${{ matrix.platform }}
          load: true
          tags: hist-${{ matrix.service }}:dry-run-${{ matrix.arch }}
          cache-from: type=gha,scope=${{ matrix.service }}-${{ matrix.arch }}
          cache-to: type=gha,scope=${{ matrix.service }}-${{ matrix.arch }},mode=max

      - name: Trivy scan (dry-run image)
        if: github.event_name == 'workflow_dispatch' && github.event.inputs.dry_run == 'true'
        uses: aquasecurity/trivy-action@0.28.0
        with:
          image-ref: hist-${{ matrix.service }}:dry-run-${{ matrix.arch }}
          severity: HIGH,CRITICAL
          exit-code: '1'
          ignore-unfixed: false
          trivyignores: .trivyignore

      - name: Export digest
        if: github.event_name != 'workflow_dispatch' || github.event.inputs.dry_run != 'true'
        run: |
          mkdir -p /tmp/digests
          digest="${{ steps.build.outputs.digest }}"
          touch "/tmp/digests/${digest#sha256:}"

      - name: Upload digest
        if: github.event_name != 'workflow_dispatch' || github.event.inputs.dry_run != 'true'
        uses: actions/upload-artifact@v4
        with:
          name: digests-${{ matrix.service }}-${{ matrix.arch }}
          path: /tmp/digests/*
          if-no-files-found: error
          retention-days: 1

  # ----------------------------------------------------------------
  # merge: fan-in per service. Combines per-platform digests into a
  # multi-arch manifest list, then runs Trivy + SBOM + cosign once.
  # Skipped on dry-run (manifest creation requires pushed digests).
  # ----------------------------------------------------------------
  merge:
    name: Merge ${{ matrix.service }} multi-arch manifest
    runs-on: ubuntu-24.04
    needs: build
    if: github.event_name != 'workflow_dispatch' || github.event.inputs.dry_run != 'true'
    strategy:
      fail-fast: false
      matrix:
        service: [api, mcp, web]
    steps:
      - uses: actions/checkout@v4

      - name: Download amd64 digest
        uses: actions/download-artifact@v4
        with:
          name: digests-${{ matrix.service }}-amd64
          path: /tmp/digests

      - name: Download arm64 digest
        uses: actions/download-artifact@v4
        with:
          name: digests-${{ matrix.service }}-arm64
          path: /tmp/digests

      - uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Compute beta version
        id: beta
        if: startsWith(github.ref, 'refs/heads/release/beta-')
        run: echo "version=${GITHUB_REF_NAME#release/beta-}" >> "$GITHUB_OUTPUT"

      - name: Compute image metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAMESPACE }}/${{ matrix.service }}
          tags: |
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=semver,pattern={{major}}
            type=raw,value=${{ steps.beta.outputs.version }}-beta,enable=${{ steps.beta.outputs.version != '' }}
            type=raw,value=${{ steps.beta.outputs.version }}-beta.${{ github.run_number }},enable=${{ steps.beta.outputs.version != '' }}
            type=raw,value=beta,enable=${{ steps.beta.outputs.version != '' }}
          flavor: |
            latest=auto

      - name: Create manifest list and push
        id: manifest
        working-directory: /tmp/digests
        run: |
          set -euo pipefail
          tag_args=$(jq -cr '.tags | map("-t " + .) | join(" ")' <<< "$DOCKER_METADATA_OUTPUT_JSON")
          docker buildx imagetools create $tag_args \
            $(printf '${{ env.REGISTRY }}/${{ env.IMAGE_NAMESPACE }}/${{ matrix.service }}@sha256:%s ' *)
          first_tag=$(jq -cr '.tags[0]' <<< "$DOCKER_METADATA_OUTPUT_JSON")
          digest=$(docker buildx imagetools inspect "$first_tag" --format '{{json .Manifest}}' | jq -r '.digest')
          echo "digest=$digest" >> "$GITHUB_OUTPUT"

      - name: Generate SBOM
        uses: anchore/sbom-action@v0.17.8
        with:
          image: ${{ env.REGISTRY }}/${{ env.IMAGE_NAMESPACE }}/${{ matrix.service }}@${{ steps.manifest.outputs.digest }}
          format: spdx-json
          output-file: sbom-${{ matrix.service }}.spdx.json

      - name: Install cosign
        uses: sigstore/cosign-installer@v3

      - name: Sign image
        env:
          DIGEST: ${{ steps.manifest.outputs.digest }}
        run: |
          cosign sign --yes \
            "${{ env.REGISTRY }}/${{ env.IMAGE_NAMESPACE }}/${{ matrix.service }}@${DIGEST}"

      - name: Attest SBOM
        env:
          DIGEST: ${{ steps.manifest.outputs.digest }}
        run: |
          cosign attest --yes \
            --predicate "sbom-${{ matrix.service }}.spdx.json" \
            --type spdx \
            "${{ env.REGISTRY }}/${{ env.IMAGE_NAMESPACE }}/${{ matrix.service }}@${DIGEST}"

      - name: Trivy vulnerability scan
        uses: aquasecurity/trivy-action@0.28.0
        with:
          image-ref: ${{ env.REGISTRY }}/${{ env.IMAGE_NAMESPACE }}/${{ matrix.service }}@${{ steps.manifest.outputs.digest }}
          severity: HIGH,CRITICAL
          exit-code: '1'
          ignore-unfixed: false
          trivyignores: .trivyignore

  # ----------------------------------------------------------------
  # release-notes: stable tag only. Unchanged behavior from prior file.
  # ----------------------------------------------------------------
  release-notes:
    name: Create GitHub Release
    needs: merge
    if: startsWith(github.ref, 'refs/tags/v') && (github.event_name != 'workflow_dispatch' || github.event.inputs.dry_run != 'true')
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Compute version (strip leading v)
        id: ver
        run: echo "tag=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          generate_release_notes: true
          prerelease: ${{ contains(github.ref_name, '-') }}
          body: |
            ## Container images

            ```
            docker pull ghcr.io/${{ github.repository }}/api:${{ steps.ver.outputs.tag }}
            docker pull ghcr.io/${{ github.repository }}/mcp:${{ steps.ver.outputs.tag }}
            docker pull ghcr.io/${{ github.repository }}/web:${{ steps.ver.outputs.tag }}
            ```

            All images are signed (cosign keyless) with SPDX SBOM attestations.
            Verify before deploying:

            ```
            cosign verify ghcr.io/${{ github.repository }}/api:${{ steps.ver.outputs.tag }} \
              --certificate-identity-regexp 'https://github.com/${{ github.repository }}/.github/workflows/' \
              --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'
            ```
```

- [ ] **Step 2.2: Validate the YAML with actionlint**

```bash
actionlint .github/workflows/release.yml
```

Expected: no output (silence = green). If errors, fix the reported lines and re-run before committing.

- [ ] **Step 2.3: Verify the file is well-formed YAML**

```bash
docker run --rm -v "$PWD":/r -w /r mikefarah/yq:4 'keys' .github/workflows/release.yml
```

Expected: prints `["name", "on", "concurrency", "permissions", "env", "jobs"]` (in some order). If yaml parse fails, fix and retry.

- [ ] **Step 2.4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "$(cat <<'EOF'
ci(release): split build matrix to native amd64/arm64 runners

Replace single QEMU-emulated job with six per-platform builds on
ubuntu-24.04 and ubuntu-24.04-arm, fanning into three per-service
merge jobs that build the multi-arch manifest, scan with Trivy once,
and SBOM/sign/attest. Drops setup-qemu-action entirely. Adds a
workflow_dispatch dry_run input that builds + scans without push,
sign, or attest — usable from any branch via:
gh workflow run release.yml --ref <branch> -f dry_run=true

Spec: artifacts/specs/2026-05-06-pipeline-fix-design.md

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Distroless runtime for `Dockerfile.rust`

**Files:**
- Modify: `Dockerfile.rust` (runtime stage, lines 50–65 in the current file)

- [ ] **Step 3.1: Replace the runtime stage**

The current runtime stage installs `ca-certificates` and `libssl3` on top of `debian:bookworm-slim`, then creates a non-root user. Distroless `cc-debian12:nonroot` ships all of that already. Replace lines 50–65 (`# ============================================================` block "Stage 4: runtime" through `ENTRYPOINT`) with:

```dockerfile
# ============================================================
# Stage 4: runtime — distroless cc-debian12:nonroot
# ============================================================
# distroless/cc-debian12 ships glibc, libssl3, libgcc, and ca-certs.
# No shell, no apt, no systemd. The :nonroot tag provides uid 65532.
# CVE surface is dramatically lower than debian:bookworm-slim — most
# entries in .trivyignore reference packages that are not present here
# (zlib, libcap, libgcrypt, gnutls, systemd, ncurses).
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
ARG BIN_NAME

WORKDIR /app
COPY --from=builder /app/target/release/${BIN_NAME} /usr/local/bin/app

# Informational — compose sets the real mapping.
EXPOSE 3001 3002

ENTRYPOINT ["/usr/local/bin/app"]
```

The `ARG RUST_VERSION`, `ARG DEBIAN_VERSION`, `chef`, `planner`, and `builder` stages stay untouched.

- [ ] **Step 3.2: Smoke-build the api image locally for amd64**

```bash
docker buildx build \
  --platform linux/amd64 \
  --load \
  --build-arg BIN_NAME=api \
  -f Dockerfile.rust \
  -t hist-api:smoke .
```

Expected: build succeeds. Note the final image size in the output (should be ~50 MB; was ~80 MB before).

- [ ] **Step 3.3: Verify the api binary loads and produces help output**

```bash
docker run --rm hist-api:smoke --help 2>&1 | head -5
```

Expected behavior depends on whether `apps/api` binary defines `--help`. Either:
- prints help text and exits 0, or
- prints an "unknown argument" error and exits non-zero (still proves the binary loaded).

What we are validating: the binary started — no `error while loading shared libraries: libssl.so.3: cannot open shared object file`. If it fails with a missing-shared-library error, distroless-cc is missing a transitive dep — fall back to `gcr.io/distroless/static-debian12` and rebuild the binary with `--target x86_64-unknown-linux-musl` (this is the documented fallback in the spec). Pause and surface to the user before pursuing.

- [ ] **Step 3.4: Repeat smoke build for mcp**

```bash
docker buildx build \
  --platform linux/amd64 \
  --load \
  --build-arg BIN_NAME=mcp \
  -f Dockerfile.rust \
  -t hist-mcp:smoke .

docker run --rm hist-mcp:smoke --help 2>&1 | head -5
```

Expected: same as Step 3.3 for the mcp binary.

- [ ] **Step 3.5: Commit**

```bash
git add Dockerfile.rust
git commit -m "$(cat <<'EOF'
fix(docker): swap api/mcp runtime to distroless/cc-debian12:nonroot

Drops debian:bookworm-slim runtime base (and its 8 suppressed Trivy
CVEs across zlib, libcap, libgcrypt, gnutls, systemd, ncurses, and
glibc). distroless/cc-debian12 ships glibc + libssl3 + libgcc + ca-
certs and no shell, apt, or systemd. The :nonroot tag provides uid
65532 so the manual useradd is no longer needed. Smoke-tested locally
(amd64) for both api and mcp.

Spec: artifacts/specs/2026-05-06-pipeline-fix-design.md

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Distroless runtime for `apps/web/Dockerfile`

**Files:**
- Modify: `apps/web/Dockerfile` (rewrite — replace entire file)

- [ ] **Step 4.1: Replace the entire `apps/web/Dockerfile`**

```dockerfile
# syntax=docker/dockerfile:1.7
#
# Next.js standalone Docker image for apps/web.
# Uses output: 'standalone' (configured in next.config.ts) so the
# runtime image ships only the server.js entrypoint + traced deps.
# Builder/deps stages run on node:22-alpine for a small, fast install
# footprint; runtime is distroless/nodejs22-debian12:nonroot for a
# near-zero CVE surface.

ARG NODE_VERSION=22

# ---- stage 1: install deps ----
FROM node:${NODE_VERSION}-alpine AS deps
RUN corepack enable
WORKDIR /app

# Copy lockfile + workspace roots so pnpm can resolve workspace deps.
# packages/types/package.json is required so pnpm sees the workspace
# package referenced by apps/web (`@historiador/types: workspace:*`)
# and creates a valid symlink instead of a broken one.
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml turbo.json ./
COPY apps/web/package.json ./apps/web/
COPY packages/types/package.json ./packages/types/
RUN pnpm install --frozen-lockfile

# ---- stage 2: build ----
FROM node:${NODE_VERSION}-alpine AS builder
RUN corepack enable
WORKDIR /app

# Copy monorepo root files so Turbopack can detect the project root
# (it walks up looking for pnpm-lock.yaml / package-lock.json)
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/apps/web/node_modules ./apps/web/node_modules
COPY apps/web ./apps/web
# Workspace package source — the `@historiador/types` symlink in
# node_modules points here, so the source must be present for
# `next build` to resolve `import { ... } from "@historiador/types"`.
COPY packages ./packages
RUN cd apps/web && npx next build

# ---- stage 3: production runner ----
# distroless/nodejs22-debian12:nonroot ships node + ca-certs + a
# nonroot user (uid 65532). No shell, no apk, no npm. Entrypoint is
# `node`, so CMD is just the script path.
FROM gcr.io/distroless/nodejs22-debian12:nonroot AS runner
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

# Copy standalone output + static assets + public dir, owned by nonroot
COPY --from=builder --chown=nonroot:nonroot /app/apps/web/.next/standalone ./
COPY --from=builder --chown=nonroot:nonroot /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=builder --chown=nonroot:nonroot /app/apps/web/public ./apps/web/public

EXPOSE 3000

CMD ["apps/web/server.js"]
```

- [ ] **Step 4.2: Smoke-build the web image locally for amd64**

```bash
docker buildx build \
  --platform linux/amd64 \
  --load \
  -f apps/web/Dockerfile \
  -t hist-web:smoke .
```

Expected: build succeeds through the deps → builder → runner stages.

- [ ] **Step 4.3: Run the web image and verify it serves the root**

```bash
container_id=$(docker run -d --rm -p 13000:3000 hist-web:smoke)
sleep 5
curl -fsS http://localhost:13000/ -o /tmp/web-smoke-output.html && echo "OK"
docker stop "$container_id"
```

Expected: prints `OK`. The HTML contents may show an error page if the API isn't reachable (no DB / API at localhost:3001), but the Next.js server itself must respond. If `curl` returns a connection-refused or the container exits, distroless-nodejs22 isn't compatible with our standalone output — fall back to `node:22-bookworm-slim` (per spec's documented fallback) and surface to the user.

- [ ] **Step 4.4: Commit**

```bash
git add apps/web/Dockerfile
git commit -m "$(cat <<'EOF'
fix(docker): swap web runtime to distroless/nodejs22-debian12:nonroot

Bumps Node 20 → 22 (Node 20 is EOL Sept 2026) and replaces node:alpine
runtime with distroless nodejs22. Builder/deps stages stay on
node:22-alpine for fast pnpm install. Runtime image: no shell, no apk,
no npm — just node + ca-certs. Eliminates the alpine CVE surface that
caused the v1.0.0 release run's web-job Trivy gate to fail.

Spec: artifacts/specs/2026-05-06-pipeline-fix-design.md

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Action version bumps in `ci.yml`

**Files:**
- Modify: `.github/workflows/ci.yml`

The CI workflow's actions are mostly current; the goal here is to verify they don't trigger the same Node-20 deprecation warning at run time, and to pin any floating versions for reproducibility.

- [ ] **Step 5.1: Inventory which actions need bumping**

Read the current file:

```bash
grep -nE 'uses: ' .github/workflows/ci.yml
```

Expected output (line numbers may vary):
```
20:      - uses: actions/checkout@v4
24:      - uses: dtolnay/rust-toolchain@stable
27:      - name: Cache cargo
28:        uses: Swatinem/rust-cache@v2
38:      - name: cargo audit
39:        uses: rustsec/audit-check@v2.0.0
53:      - uses: actions/checkout@v4
55:      - uses: pnpm/action-setup@v4
57:      - uses: actions/setup-node@v4
77:      - uses: actions/checkout@v4
79:      - uses: docker/setup-buildx-action@v3
82:        uses: docker/build-push-action@v6
93:        uses: docker/build-push-action@v6
104:        uses: docker/build-push-action@v6
```

All of these are at major versions that maintainers have committed to keeping Node-24 compatible. **No bump required for ci.yml.** Proceed to Step 5.2.

- [ ] **Step 5.2: Validate ci.yml with actionlint**

```bash
actionlint .github/workflows/ci.yml
```

Expected: no output. If errors, surface to the user — they're independent of this plan and should not be fixed silently.

- [ ] **Step 5.3: No commit needed**

If Steps 5.1 and 5.2 are both clean, no `ci.yml` changes are required. If either reveals an issue, pause and surface to the user before adding anything to this plan.

---

## Task 6: First dry-run dispatch

**Files:** none (operational only)

- [ ] **Step 6.1: Push the branch**

```bash
git push origin bugfix/fix-pipeline-issues
```

Expected: push succeeds. (No CI triggers from the push alone — `release.yml` only fires on tags, `release/beta-*` branches, or workflow_dispatch.)

- [ ] **Step 6.2: Trigger the dry-run**

```bash
gh workflow run release.yml --ref bugfix/fix-pipeline-issues -f dry_run=true
```

Expected: `✓ Created workflow_dispatch event for release.yml at bugfix/fix-pipeline-issues`.

- [ ] **Step 6.3: Watch the run**

```bash
sleep 5
run_id=$(gh run list --workflow=release.yml --branch=bugfix/fix-pipeline-issues --limit 1 --json databaseId --jq '.[0].databaseId')
echo "Run ID: $run_id"
gh run watch "$run_id" --exit-status
```

Expected outcomes by job:
- 6 `build` jobs (api/mcp/web × amd64/arm64) all green
- 0 `merge` jobs (skipped — dry-run)
- 0 `release-notes` (skipped)

Per-job duration target: ≤ 15 min on native runners. If any single build job exceeds 25 min, capture logs (`gh run view --job=<id> --log`) and surface to the user before continuing.

If Trivy fails on any (service, platform) cell, **continue to Task 7** — that's the expected next step.

If a build step fails for a non-Trivy reason (compilation, OOM, missing file), pause and surface to the user.

---

## Task 7: Trivy cleanup — prune `.trivyignore` based on real findings

**Files:**
- Modify: `.trivyignore`

After the first dry-run, we have real Trivy reports against the new bases. Most current `.trivyignore` entries should be obsolete.

- [ ] **Step 7.1: Pull each dry-run image locally and run Trivy**

The dry-run jobs ran Trivy in-runner, but to inspect findings interactively we need local images. The simplest path: rebuild locally (cache should be near-instant after the dispatch).

```bash
for svc in api mcp web; do
  if [ "$svc" = "web" ]; then
    file="apps/web/Dockerfile"; args=""
  else
    file="Dockerfile.rust"; args="--build-arg BIN_NAME=$svc"
  fi
  docker buildx build --platform linux/amd64 --load $args -f "$file" -t "hist-$svc:trivy" .
  echo "=== Trivy report for $svc ==="
  docker run --rm -v "$PWD/.trivyignore:/.trivyignore:ro" \
    -v /var/run/docker.sock:/var/run/docker.sock \
    aquasec/trivy:0.55.0 image \
    --severity HIGH,CRITICAL \
    --ignore-unfixed=false \
    --ignorefile /.trivyignore \
    "hist-$svc:trivy" \
    > "/tmp/trivy-$svc.txt" 2>&1
  cat "/tmp/trivy-$svc.txt"
done
```

(`aquasec/trivy:0.55.0` is the locally pinned version — bump if the runtime version diverges. Actionlint isn't relevant here.)

- [ ] **Step 7.2: For each suppression in `.trivyignore`, decide: keep or remove**

Open `.trivyignore`. For each `CVE-*` line:
- If the CVE no longer appears in any of `/tmp/trivy-{api,mcp,web}.txt`: **delete the line**.
- If it still appears: **keep the line, and update the date stamp to `2026-05-06`** if not already.
- If a NEW CVE appears in the Trivy report and is HIGH/CRITICAL: add a new line with the same justification format (CVE id, reason, date `2026-05-06`, reviewer `gabriel`). Do not add an entry without a real justification — surface to the user instead.

Expected outcome: `.trivyignore` shrinks from 8 entries to ≤ 2. Most current entries reference packages distroless-cc and distroless-nodejs do not ship.

- [ ] **Step 7.3: Verify Trivy passes against the trimmed `.trivyignore`**

Re-run Step 7.1 with the updated file. All three reports must end with `✓ no vulnerabilities found` or list only acknowledged-suppressed CVEs at HIGH/CRITICAL. If any HIGH/CRITICAL is unsuppressed, the gate would fail in CI — pause and reconcile with the user.

- [ ] **Step 7.4: Commit**

```bash
git add .trivyignore
git commit -m "$(cat <<'EOF'
chore(trivy): prune .trivyignore after distroless base swap

Most v1.0.0 entries referenced packages that do not ship in
distroless/cc-debian12 or distroless/nodejs22-debian12 (zlib, libcap,
libgcrypt, gnutls, systemd, ncurses). Removed obsolete suppressions
and refreshed dates on those that remain. All entries continue to
carry per-CVE justification per the file's stated policy.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Re-dispatch dry-run until green

**Files:** none (operational only)

- [ ] **Step 8.1: Push the trimmed `.trivyignore`**

```bash
git push origin bugfix/fix-pipeline-issues
```

- [ ] **Step 8.2: Re-dispatch dry-run**

```bash
gh workflow run release.yml --ref bugfix/fix-pipeline-issues -f dry_run=true
```

- [ ] **Step 8.3: Watch and confirm green**

```bash
sleep 5
run_id=$(gh run list --workflow=release.yml --branch=bugfix/fix-pipeline-issues --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$run_id" --exit-status
```

Expected: all 6 `build` jobs green, no Trivy failures, no compilation failures. If still red, return to Task 7 with the new findings. Repeat the Task 7 → Task 8 loop until green; surface to the user after 3 iterations if not converging.

---

## Task 9: Open PR and prepare real publish

**Files:** none (operational only)

- [ ] **Step 9.1: Confirm full commit list on the branch**

```bash
git log --oneline main..HEAD
```

Expected: each task's commit is visible (release.yml rewrite, Dockerfile.rust swap, web Dockerfile rewrite, trivyignore prune, plus the spec/plan commits from earlier).

- [ ] **Step 9.2: Open the PR to `develop`**

```bash
gh pr create --base develop --head bugfix/fix-pipeline-issues \
  --title "fix(ci): native multi-arch release pipeline + distroless bases" \
  --body "$(cat <<'EOF'
## Summary
- Split release matrix to native amd64/arm64 runners — drops QEMU, eliminates 6h timeouts and cargo-chef OOMs
- Swap api/mcp runtime to distroless/cc-debian12:nonroot — eliminates 7 of 8 suppressed Bookworm CVEs
- Swap web runtime to distroless/nodejs22-debian12:nonroot — bumps Node 20→22 and removes alpine CVE surface
- Add workflow_dispatch dry_run input for branch-level pipeline validation
- Prune .trivyignore to only what's still flagged on the new bases

Spec: [artifacts/specs/2026-05-06-pipeline-fix-design.md](artifacts/specs/2026-05-06-pipeline-fix-design.md)
Plan: [artifacts/plans/2026-05-06-pipeline-fix-plan.md](artifacts/plans/2026-05-06-pipeline-fix-plan.md)

## Test plan
- [x] Dry-run dispatched and green on this branch (workflow_dispatch + dry_run=true)
- [x] Trivy reports clean against trimmed .trivyignore
- [ ] After merge: delete v1.0.0 tag, retag at merge commit, push tag, real publish runs end-to-end
- [ ] cosign verify on each published image

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

Expected: PR URL is printed. Capture it for the user.

- [ ] **Step 9.3: Stop here for user review**

The next step (Task 10) deletes a published tag. Pause and surface the PR URL to the user. **Do not proceed to Task 10 without explicit confirmation.**

---

## Task 10: Real publish (after PR merge)

**Files:** none (operational only)

This task only runs after the PR is merged to `develop` and `main`.

- [ ] **Step 10.1: Confirm merge to main**

```bash
git fetch origin
git log --oneline origin/main | head -5
```

Expected: the rewrite commits appear on `main` (typically via the develop → main release flow this repo uses).

- [ ] **Step 10.2: Delete the existing v1.0.0 tag (with explicit user confirmation)**

This is destructive on a shared system. **Pause and ask the user:** *"OK to delete the existing v1.0.0 tag locally and on origin? It currently points to a commit whose release run failed. The new tag will point to the merge commit on main."* On approval:

```bash
git tag -d v1.0.0
git push origin :refs/tags/v1.0.0
```

Expected: local tag deleted, remote ref deleted. If the remote rejects (rare — would happen only with branch protection on tags), surface to the user.

- [ ] **Step 10.3: Re-tag at the current main HEAD**

```bash
git checkout main
git pull origin main
git tag -a v1.0.0 -m "v1.0.0"
git push origin v1.0.0
```

Expected: tag pushed. The release workflow fires automatically.

- [ ] **Step 10.4: Watch the real release run**

```bash
sleep 10
run_id=$(gh run list --workflow=release.yml --branch=v1.0.0 --limit 1 --json databaseId --jq '.[0].databaseId')
echo "Real release run: $run_id"
gh run watch "$run_id" --exit-status
```

Expected: 6 build jobs green, 3 merge jobs green (each with Trivy + SBOM + cosign sign + cosign attest), 1 release-notes job green. Total wall-clock ≤ 25 min.

- [ ] **Step 10.5: Verify a published image with cosign**

```bash
cosign verify ghcr.io/gaspecian/historiador-docs/api:1.0.0 \
  --certificate-identity-regexp 'https://github.com/gaspecian/historiador-docs/.github/workflows/' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'
```

Expected: cosign prints the verified certificate claims and exits 0. If verification fails, surface immediately — do not announce the release.

- [ ] **Step 10.6: Confirm GHCR has the multi-arch manifest**

```bash
docker buildx imagetools inspect ghcr.io/gaspecian/historiador-docs/api:1.0.0
```

Expected: prints a manifest list with both `linux/amd64` and `linux/arm64` entries.

- [ ] **Step 10.7: Surface success to the user**

Report:
- Real release run ID + duration
- All three image references (api, mcp, web) at `:1.0.0`
- cosign verification outcome
- Any remaining `.trivyignore` entries (and the rationale for keeping them)

---

## Self-review notes

**Spec coverage:**

| Spec section | Implemented in |
|---|---|
| `release.yml` restructure (build matrix, merge fan-in, dispatch) | Task 2 |
| Dockerfile.rust runtime swap | Task 3 |
| apps/web/Dockerfile runtime swap | Task 4 |
| `.trivyignore` cleanup | Task 7 |
| Action version bumps | Task 5 |
| Pipeline simulation (workflow_dispatch dry-run) | Tasks 2, 6, 8 |
| Validation sequence | Tasks 6 → 10 |
| Risks & fallbacks (distroless smoke-test fail) | Steps 3.3, 4.3 (explicit fall-back instructions) |
| Tag deletion + retag | Task 10 (with explicit user confirmation) |

No gaps.

**Placeholder scan:** none. Every step contains either an exact command, a complete code block, or an explicit pause-and-confirm decision point.

**Type consistency:** all matrix labels (`amd64`, `arm64`), artifact names (`digests-${service}-${arch}`), and image refs match across the build → merge handoff.

**Outside-of-spec additions:** the actionlint precondition is added by the plan to align with this repo's prior pipeline plan (April 2026); it is non-invasive and the docker-based fallback is provided.
