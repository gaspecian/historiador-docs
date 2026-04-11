# syntax=docker/dockerfile:1.7
#
# Next.js standalone Docker image for apps/web.
# Uses output: 'standalone' (configured in next.config.ts) so the
# runtime image ships only the server.js entrypoint + traced deps
# — no full node_modules needed.

ARG NODE_VERSION=20

# ---- stage 1: install deps ----
FROM node:${NODE_VERSION}-alpine AS deps
RUN corepack enable
WORKDIR /app

# Copy lockfile + workspace roots so pnpm can resolve workspace deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml turbo.json ./
COPY apps/web/package.json ./apps/web/
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
RUN cd apps/web && npx next build

# ---- stage 3: production runner ----
FROM node:${NODE_VERSION}-alpine AS runner
WORKDIR /app
ENV NODE_ENV=production

RUN addgroup --system --gid 1001 nodejs \
 && adduser --system --uid 1001 nextjs

# Copy standalone output + static assets + public dir
COPY --from=builder --chown=nextjs:nodejs /app/apps/web/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=builder --chown=nextjs:nodejs /app/apps/web/public ./apps/web/public

USER nextjs

EXPOSE 3000
ENV PORT=3000
ENV HOSTNAME=0.0.0.0

CMD ["node", "apps/web/server.js"]
