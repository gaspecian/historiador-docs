"use client";

/**
 * Sprint 11 editor-v2 container (split pane + Tiptap canvas).
 *
 * This is the entry point that renders when `NEXT_PUBLIC_EDITOR_V2`
 * is true. A6 lands the real chat pane and wires the WebSocket; A10
 * layers the proposal overlay on top of the canvas.
 */

import { useCallback, useState } from "react";

import { Canvas } from "./canvas";
import { ChatPane } from "./chat";
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

export interface EditorV2Props {
  /** When absent, render a demo canvas — no WS connection. */
  pageId?: string;
  language?: string;
  /** Short-lived access token for the WS handshake. Read from
   *  localStorage by the default caller in `editor-v2-page.tsx`. */
  token?: string;
}

export function EditorV2({ pageId, language, token }: EditorV2Props = {}) {
  const [savedAt, setSavedAt] = useState<Date | null>(null);
  // Selection + cursor are tracked here so the chat pane can ride
  // them on outgoing messages. A10 wires the canvas to update these
  // whenever the Tiptap selection changes; for now they are inert.
  const [selectionText] = useState("");
  const [cursorBlockId] = useState<string | null>(null);

  const handleSave = useCallback(async (markdown: string) => {
    // TODO (A10/A11): PATCH /pages/:id with the base markdown once
    // the overlay pipeline guarantees only approved content reaches
    // this function. The A5 scaffold stamps a timestamp and logs
    // size so the UI can show a "saved" indicator.
    if (process.env.NODE_ENV !== "production") {
      console.debug(`[editor-v2] autosave scaffold: ${markdown.length} bytes`);
    }
    setSavedAt(new Date());
  }, []);

  const chatReady = Boolean(pageId && language && token);

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
        left={
          chatReady ? (
            <ChatPane
              pageId={pageId!}
              language={language!}
              token={token!}
              selectionText={selectionText}
              cursorBlockId={cursorBlockId}
            />
          ) : (
            <DemoChatPlaceholder />
          )
        }
        right={
          <div className="p-4">
            <Canvas initialMarkdown={STARTER_MARKDOWN} onSave={handleSave} />
          </div>
        }
      />
    </div>
  );
}

function DemoChatPlaceholder() {
  return (
    <div className="flex-1 flex flex-col items-center justify-center text-center p-8 gap-3">
      <p className="t-body-lg text-[var(--color-text-primary)]">AI assistant</p>
      <p className="t-body-sm text-[var(--color-text-secondary)] max-w-[260px]">
        Open a page from the dashboard to start a conversation with the
        editor agent. The chat streams through a WebSocket and carries
        your canvas state on every turn.
      </p>
    </div>
  );
}
