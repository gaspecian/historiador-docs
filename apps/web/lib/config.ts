/**
 * Frontend feature-flag surface for Sprint 11.
 *
 * `NEXT_PUBLIC_EDITOR_V2` must be read at build time (via Next.js's
 * inlining of `NEXT_PUBLIC_*` env vars). Keep the value in sync with
 * the server-side `EDITOR_V2_ENABLED` — the handshake in ADR-012 will
 * detect a mismatch, but the UI routes rely on this flag to decide
 * whether to render the split-pane canvas (Phase A5) or fall back to
 * the legacy one-shot editor.
 */

function parseFlag(raw: string | undefined): boolean {
  if (!raw) return false;
  const v = raw.trim().toLowerCase();
  return v === "1" || v === "true" || v === "yes" || v === "on";
}

export const EDITOR_V2_ENABLED = parseFlag(process.env.NEXT_PUBLIC_EDITOR_V2);
