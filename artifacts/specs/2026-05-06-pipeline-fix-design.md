# Release pipeline: native multi-arch + distroless bases

**Date:** 2026-05-06
**Author:** Gabriel Specian (with Claude Opus 4.7)
**Branch:** `bugfix/fix-pipeline-issues`
**Status:** Approved — ready for implementation plan
**Supersedes:** [2026-04-25-ghcr-release-pipeline-design.md](2026-04-25-ghcr-release-pipeline-design.md) for the build/scan layout (signing, SBOM, and release-notes flow are inherited).

## Background

The v1.0.0 release run timed out at 6 hours. The follow-up commit `def08f3` ("unblock release pipeline") bumped `rustls-webpki`, granted `checks: write`, populated `.trivyignore` with eight Debian Bookworm CVEs, and pinned QEMU to `v7.0.0-28`. The retagged run still failed:

- `web` job: Trivy gate failed at 5m04s — `node:20-alpine` carries CVEs that are not in `.trivyignore`.
- `api` job: `cargo chef cook --release` exited 101 at 14m32s — almost certainly OOM during arm64 emulation under QEMU.
- `mcp` job: still in progress at the time of design (likely to fail the same way as `api`).

The single root cause behind both speed and stability is **QEMU-emulated arm64 builds on an amd64 runner**. Rust release builds with `lto = "thin"` and `codegen-units = 1` are LLVM-heavy and routinely OOM under QEMU translation, and even when they don't, they take 5–10× longer than native. The QEMU pin (`tonistiigi/binfmt:qemu-v7.0.0-28`) is itself a workaround for a v8.x crash on Node 20 / V8 — see [release.yml](../../.github/workflows/release.yml) lines 38–44.

Beyond speed, `.trivyignore` currently *suppresses* CVEs rather than fixing them. Eight Bookworm base-image CVEs are listed, all marked "no upstream fix." A base-image swap can eliminate most of them outright, turning suppression into removal.

## Goals

1. **Publish v1.0.0 reliably** — every job finishes, no QEMU hangs, no OOM, no 6h timeouts.
2. **Eliminate suppressed CVEs** — most `.trivyignore` entries should disappear because the underlying packages are no longer in the image.
3. **Cut wall-clock time** — target ≤ 25 min end-to-end (vs. hours today / 6h timeout).
4. **Restore reproducibility** — replace `@master` and floating `@v0` action pins with explicit versions.
5. **Enable safe re-runs** — add a `workflow_dispatch` dry-run path so we can validate from a branch without polluting GHCR or shipping junk tags.

## Non-goals

- Replacing the release-notes job, signing flow, or SBOM attestation — all inherited unchanged from [ADR-style decisions in the prior spec](2026-04-25-ghcr-release-pipeline-design.md).
- Touching `ci.yml` beyond what action-version bumps strictly require. CI is fast and green today.
- Replacing `Dockerfile.rust`'s builder stage. Only the runtime stage changes.
- Building or restructuring the `vexfs` image. Out of scope per the existing comment in [ci.yml](../../.github/workflows/ci.yml) lines 113–117.
- Re-architecting to a static-musl Rust binary. Considered as a fallback only if the distroless-cc smoke test fails.

## Architecture

The new release pipeline has three layers per service:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 1: Per-platform build (matrix: service × platform)           │
│                                                                     │
│   amd64-api ─┐    amd64-mcp ─┐    amd64-web ─┐                      │
│              ├─→ runs-on: ubuntu-24.04                              │
│   arm64-api ─┤    arm64-mcp ─┤    arm64-web ─┤                      │
│              └─→ runs-on: ubuntu-24.04-arm                          │
│                                                                     │
│   Outputs: digest-by-platform-by-service                            │
│   No QEMU. No emulation. Native compilation.                        │
└─────────────────────────────────────────────────────────────────────┘
                          │
                          ▼  fan-in
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 2: Manifest + scan + sign (matrix: service)                  │
│                                                                     │
│   merge-api          merge-mcp          merge-web                   │
│   │                  │                  │                           │
│   ├─ buildx imagetools create  (combine amd64+arm64 → multi-arch)   │
│   ├─ Trivy scan                (once, on the merged tag)            │
│   ├─ Generate SBOM             (once, on the manifest digest)       │
│   ├─ cosign sign               (once)                               │
│   └─ cosign attest             (once)                               │
└─────────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  Layer 3: release-notes  (only on v* tag)                           │
│  Unchanged from current implementation.                             │
└─────────────────────────────────────────────────────────────────────┘
```

Six parallel build jobs, three fan-in jobs, one optional release-notes job. Wall-clock = max(build) + max(merge) + release-notes ≈ 15 + 5 + 2 = **~22 minutes** target.

## Detailed design

### release.yml restructure

**Triggers:**

```yaml
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
```

**Jobs:**

1. **`build`** — matrix `service ∈ {api, mcp, web} × platform ∈ {linux/amd64, linux/arm64}`.
   - `runs-on` chosen by platform: `ubuntu-24.04` for amd64, `ubuntu-24.04-arm` for arm64.
   - `docker/setup-qemu-action` — **removed**. Native runners need no emulation.
   - `docker/setup-buildx-action@v3`, `docker/login-action@v3` — unchanged (already current).
   - `docker/build-push-action@v6` with `platforms: ${{ matrix.platform }}` (single platform), `push: ${{ inputs.dry_run != true }}`, cache scoped per `(service, platform)`.
   - Outputs: `digest` for downstream merge job.

2. **`merge`** — matrix `service ∈ {api, mcp, web}`. Depends on `build`.
   - Skipped entirely when `inputs.dry_run == true` (manifest creation requires pushed digests).
   - `docker/metadata-action@v5` produces tags from semver / branch / beta computation (logic preserved from current `release.yml`).
   - `docker buildx imagetools create` joins the per-platform digests into the multi-arch tag.
   - Trivy scan runs **once** against the merged tag (not per platform — avoids double failures and double cost).
   - Anchore SBOM action runs once on the merged tag's digest.
   - cosign sign + attest run once on the merged tag's digest.

3. **`release-notes`** — unchanged. Triggered only on `v*` tag push. Skipped on `workflow_dispatch` and on `release/beta-*` branches.

**Dry-run semantics:** when `inputs.dry_run == true`:
- Each `build` job sets `push: false` and `load: true`, leaving the just-built single-platform image in the local Docker engine.
- Each `build` job then runs Trivy directly against that local image (`image-ref: hist-${{ matrix.service }}:dry-run-${{ matrix.platform }}`) so we still get a CVE signal per (service, platform) without needing the manifest. This gives 6 Trivy reports per run.
- The `merge` job is skipped (no digests in GHCR to merge), and so are SBOM, cosign sign, and cosign attest — those are validated by the real publish run, not the dry-run.

This means dry-run validates: native build success on both platforms, build duration, and CVE posture. It does not validate: manifest creation, SBOM, signing. Those are inherently push-coupled and only meaningful on a real publish.

### Dockerfile.rust runtime swap

Current runtime stage (lines 50–65):

```dockerfile
FROM debian:${DEBIAN_VERSION}-slim AS runtime
ARG BIN_NAME
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -s /sbin/nologin app
WORKDIR /app
COPY --from=builder /app/target/release/${BIN_NAME} /usr/local/bin/app
USER app
EXPOSE 3001 3002
ENTRYPOINT ["/usr/local/bin/app"]
```

New runtime stage:

```dockerfile
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
ARG BIN_NAME
WORKDIR /app
COPY --from=builder /app/target/release/${BIN_NAME} /usr/local/bin/app
EXPOSE 3001 3002
ENTRYPOINT ["/usr/local/bin/app"]
```

- `cc-debian12` provides `libc`, `libssl3`, `libgcc`, and `ca-certificates`. No `apt`, no shell, no systemd.
- `:nonroot` tag ships a `nonroot` user (uid 65532) — drops the manual `useradd`.
- The `apt-get install` is no longer needed because everything is in the base.
- Image size: ~50 MB (binary + base). Down from `bookworm-slim` ~80 MB and dramatically lower CVE surface.
- The `pkg-config libssl-dev` install in the **builder** stage stays — it's a build-time dep for crates that wrap OpenSSL headers (transitive only; we use rustls at runtime). Builder stage is not shipped.

**Smoke test (local, before pushing):** `docker run --rm <image> --help` must exit 0 (or with the binary's normal help-output behavior). If the binary fails to load due to a missing dynamic library, fall back to `gcr.io/distroless/static-debian12` and rebuild with `--target x86_64-unknown-linux-musl` (this is the documented fallback; do not pursue without first failing on `cc-debian12`).

### apps/web/Dockerfile runtime swap

Current runner stage (lines 42–60):

```dockerfile
FROM node:${NODE_VERSION}-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production
RUN addgroup --system --gid 1001 nodejs \
 && adduser --system --uid 1001 nextjs
COPY --from=builder --chown=nextjs:nodejs /app/apps/web/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=builder --chown=nextjs:nodejs /app/apps/web/public ./apps/web/public
USER nextjs
EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME=0.0.0.0
CMD ["node", "apps/web/server.js"]
```

New runner stage:

```dockerfile
FROM gcr.io/distroless/nodejs22-debian12:nonroot AS runner
WORKDIR /app
ENV NODE_ENV=production
COPY --from=builder --chown=nonroot:nonroot /app/apps/web/.next/standalone ./
COPY --from=builder --chown=nonroot:nonroot /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=builder --chown=nonroot:nonroot /app/apps/web/public ./apps/web/public
EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME=0.0.0.0
CMD ["apps/web/server.js"]
```

- Distroless `nodejs22-debian12` ships `node` + ca-certificates + a `nonroot` user. No shell, no `apk`, no `npm`.
- `CMD` drops the explicit `node` invocation — distroless-nodejs's entrypoint is `node`, so `CMD ["apps/web/server.js"]` runs `node apps/web/server.js`.
- `ARG NODE_VERSION` bumps from `20` to `22` for the `deps` and `builder` stages — Node 20 is EOL Sept 2026 and the current run already shows the deprecation warning.
- Builder/deps stages stay on `node:22-alpine` (small, fast, doesn't ship in the final image).

**Smoke test:** `docker run --rm -p 3000:3000 <image>` followed by `curl localhost:3000` must serve the Next.js root.

### .trivyignore cleanup

After the base swap, locally run:

```bash
trivy image --severity HIGH,CRITICAL --ignore-unfixed=false ghcr.io/.../api:dry-run
```

For each suppression in [.trivyignore](../../.trivyignore), if the CVE no longer appears, **remove the line**. For any new CVEs flagged by Trivy against `cc-debian12` or `nodejs22-debian12`, add an entry only with the same justification rigor as the existing file (CVE id, reason, date `2026-05-06`, reviewer `gabriel`).

Expected outcome: `.trivyignore` shrinks from 8 entries to ≤ 2. The current 8 entries reference packages that distroless-cc doesn't ship: zlib, libcap, libgcrypt, gnutls, systemd, ncurses. Glibc is the only one that may persist.

### Action version bumps

| Action | Current | Action |
|---|---|---|
| `actions/checkout` | v4 | bump to **v5** if released and Node-24 compatible; otherwise stay on v4 with workflow-level `env: FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: "true"`. Implementation phase verifies. |
| `docker/setup-qemu-action` | v3 (pinned) | **remove** from `release.yml` |
| `aquasecurity/trivy-action` | `@master` | pin to a specific tag (e.g. `@0.28.0`); implementation phase picks the current latest stable release. |
| `anchore/sbom-action` | `@v0` | pin to a specific `v0.x.y` tag; implementation phase picks the current latest. |
| All others | various | already current — verify in `release.yml` and `ci.yml` and leave unless a Node-24 deprecation specifically applies. |

This eliminates the Node-20-deprecation banner that GHA emits (June-2-2026 cutover, Sept-16-2026 removal).

### Pipeline simulation strategy

The new `workflow_dispatch` trigger is the simulation path:

```bash
# Push the branch with the rewritten workflow + dockerfiles:
git push origin bugfix/fix-pipeline-issues

# Trigger a dry-run (build + scan, no push, no sign):
gh workflow run release.yml --ref bugfix/fix-pipeline-issues -f dry_run=true

# Watch:
gh run watch
```

Local simulation with `act` is rejected: `act` does not realistically model `ubuntu-24.04-arm` runners (the whole point of this change), GHCR auth, or cosign keyless OIDC. `workflow_dispatch` from a branch is the canonical GHA pattern for this.

## Validation sequence (rollout order)

1. Rewrite [.github/workflows/release.yml](../../.github/workflows/release.yml) per [release.yml restructure](#releaseyml-restructure).
2. Rewrite the runtime stage of [Dockerfile.rust](../../Dockerfile.rust) per [Dockerfile.rust runtime swap](#dockerfilerust-runtime-swap).
3. Rewrite the runner stage of [apps/web/Dockerfile](../../apps/web/Dockerfile) per [apps/web/Dockerfile runtime swap](#appsweb-dockerfile-runtime-swap).
4. Bump action versions in both `release.yml` and `ci.yml` per [Action version bumps](#action-version-bumps).
5. Local: `docker buildx build --platform linux/amd64 -f Dockerfile.rust --build-arg BIN_NAME=api -t hist-api:test .` then `docker run --rm hist-api:test --help`. Repeat for `mcp` and the web image.
6. Commit + push branch.
7. `gh workflow run release.yml --ref bugfix/fix-pipeline-issues -f dry_run=true`. Confirm all 6 build jobs go green; record durations.
8. Pull the dry-run images locally and run `trivy image` against each. Update `.trivyignore` to match reality (prune dead entries, add any new ones with full justification).
9. Re-dispatch dry-run until green end-to-end.
10. Open PR `bugfix/fix-pipeline-issues` → `develop` for review.
11. After merge to `main`: delete the existing `v1.0.0` tag (`git tag -d v1.0.0 && git push origin :refs/tags/v1.0.0`), re-tag at the merge commit, push the tag — real publish runs.

## Risks & fallbacks

- **distroless-cc-debian12 missing a dyn lib our binary needs.** Fallback: `gcr.io/distroless/static-debian12` + rebuild with `--target x86_64-unknown-linux-musl` and `--target aarch64-unknown-linux-musl`. Detected by step 5's smoke test.
- **distroless-nodejs22 unable to load Next.js standalone server.** Fallback: `node:22-bookworm-slim` (still bumps off Alpine, still drops Node 20, accepts a slightly larger CVE surface). Detected by step 5's web smoke test.
- **`ubuntu-24.04-arm` runner queue depth.** GA but newer than `ubuntu-24.04`. If queue times become noticeable, the matrix still works; we just lose some wall-clock parallelism. Mitigated by GHA's free arm64 capacity for public repos.
- **Tag deletion on `v1.0.0`.** Step 11 deletes and recreates a tag. Acceptable because the previous tag's release run failed and no images were ever published — there is nothing to invalidate. If GHCR has stray manifests from the failed run, they will be overwritten by the new run.

## Out of scope (not addressed by this spec)

- A static-musl Rust build (mentioned only as a fallback path).
- VexFS image build (existing exclusion).
- Any change to `apps/api`, `apps/mcp`, `apps/web`, or `crates/*` source code. This is purely a build/release change.
- Rotating cosign or GHCR credentials.
