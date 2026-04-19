"use client";

/**
 * Two-pane editor shell per ADR-008.
 *
 * Left pane: persistent WebSocket-connected conversation (built in
 * A6). Right pane: live canvas (A5, this phase).
 *
 * The layout uses a CSS grid so the proportions stay stable across
 * viewport sizes, with a hard min-width on the chat to keep long
 * AI turns readable.
 */

import type { ReactNode } from "react";

export interface SplitPaneProps {
  left: ReactNode;
  right: ReactNode;
}

export function SplitPane({ left, right }: SplitPaneProps) {
  return (
    <div className="grid grid-cols-[360px_1fr] gap-4 min-h-[calc(100vh-var(--topbar-height))] p-4">
      <aside className="rounded-[var(--radius-lg)] bg-[var(--color-surface-subtle)] border border-[var(--color-surface-border)] flex flex-col overflow-hidden min-w-[320px]">
        {left}
      </aside>
      <main className="rounded-[var(--radius-lg)] bg-[var(--color-surface-canvas)] border border-[var(--color-surface-border)] overflow-auto">
        {right}
      </main>
    </div>
  );
}
