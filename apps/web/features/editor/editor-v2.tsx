"use client";

/**
 * Sprint 11 editor-v2 container (split pane + Tiptap canvas).
 *
 * This is the entry point that renders when `NEXT_PUBLIC_EDITOR_V2`
 * is true. A6 lands the real chat pane and wires the WebSocket; A10
 * layers the proposal overlay on top of the canvas.
 */

import { useCallback, useMemo, useState } from "react";
import { Sparkles } from "lucide-react";

import { Canvas } from "./canvas";
import { SplitPane } from "./split-pane";

const STARTER_MARKDOWN = `<!-- block:01966000-0000-7000-8000-000000000001 -->

# Historiador AI Editor

<!-- block:01966000-0000-7000-8000-000000000002 -->

Start typing to draft your page. The canvas is block-aware — every
top-level element carries a stable ID so the AI assistant can
propose precise edits without overwriting your work.

<!-- block:01966000-0000-7000-8000-000000000003 -->

## What comes next

<!-- block:01966000-0000-7000-8000-000000000004 -->

The chat pane on the left will become interactive once A6 lands
the WebSocket wiring and canvas-aware context assembly.
`;

export function EditorV2() {
  const [savedAt, setSavedAt] = useState<Date | null>(null);

  const handleSave = useCallback(async (markdown: string) => {
    // TODO (A6): PATCH /pages/:id with the base markdown. For the
    // A5 scaffold we stamp a timestamp and log size so the UI can
    // show a "saved" indicator and diagnostics can observe payloads.
    if (process.env.NODE_ENV !== "production") {
      console.debug(`[editor-v2] autosave scaffold: ${markdown.length} bytes`);
    }
    setSavedAt(new Date());
  }, []);

  const chatPlaceholder = useMemo(
    () => (
      <div className="flex-1 flex flex-col items-center justify-center text-center p-8 gap-3">
        <Sparkles className="w-6 h-6 text-[var(--color-primary-600)]" aria-hidden />
        <p className="t-body-lg text-[var(--color-text-primary)]">AI assistant</p>
        <p className="t-body-sm text-[var(--color-text-secondary)] max-w-[240px]">
          Conversation arrives in the next phase. For now, you can edit
          the canvas directly — every block gets a stable ID.
        </p>
      </div>
    ),
    []
  );

  return (
    <div>
      <header className="flex items-center justify-between px-4 h-[var(--topbar-height)] border-b border-[var(--color-surface-border)] bg-[var(--color-surface-page)]">
        <h1 className="t-label text-[var(--color-text-tertiary)] tracking-[var(--text-caps-tracking)] uppercase">
          Editor v2
        </h1>
        <span className="t-body-sm text-[var(--color-text-tertiary)]">
          {savedAt ? `Saved ${savedAt.toLocaleTimeString()}` : "Not saved yet"}
        </span>
      </header>
      <SplitPane
        left={chatPlaceholder}
        right={
          <div className="p-4">
            <Canvas initialMarkdown={STARTER_MARKDOWN} onSave={handleSave} />
          </div>
        }
      />
    </div>
  );
}
