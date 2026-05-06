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
