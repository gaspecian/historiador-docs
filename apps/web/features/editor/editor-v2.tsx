"use client";

/**
 * Sprint 11 editor-v2 container (split pane + Tiptap canvas).
 *
 * This is the entry point that renders when `NEXT_PUBLIC_EDITOR_V2`
 * is true. A6 lands the real chat pane and wires the WebSocket; A10
 * layers the proposal overlay on top of the canvas.
 */

import { useCallback, useState } from "react";

import type { AutonomyMode } from "@/lib/editor-ws";

import { AutonomySelector } from "./autonomy";
import { Canvas } from "./canvas";
import { ChatPane } from "./chat";
import { ProposalPanel, summariseOp, useProposalStore } from "./overlay";
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
  // them on outgoing messages. Wiring the Tiptap selection listener
  // (to update these) lands with B2's inline toolbar.
  const [selectionText] = useState("");
  const [cursorBlockId] = useState<string | null>(null);
  const [autonomyMode, setAutonomyMode] = useState<AutonomyMode>("propose");

  const proposals = useProposalStore();

  const handleAutonomyChange = useCallback((next: AutonomyMode) => {
    setAutonomyMode(next);
    window.dispatchEvent(
      new CustomEvent("historiador:autonomy-change", { detail: { mode: next } })
    );
  }, []);

  const handleProposal = useCallback(
    (proposalId: string, op: unknown) => {
      const { summary, kind, rationale } = summariseOp(op);
      proposals.add({
        proposalId,
        kind,
        summary,
        rationale,
        raw: op,
      });
    },
    [proposals]
  );

  const handleSave = useCallback(async (markdown: string) => {
    // The proposal overlay lives outside the canvas's base state, so
    // `markdown` here is the user-approved base only — this is the
    // ADR-013 "unapproved content never auto-saves" invariant,
    // enforced by the type signature (Canvas.onSave receives only
    // the base serialisation).
    if (process.env.NODE_ENV !== "production") {
      console.debug(`[editor-v2] autosave base: ${markdown.length} bytes`);
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
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={() =>
              window.dispatchEvent(new CustomEvent("historiador:review-request"))
            }
            disabled={!chatReady}
            className="t-body-sm px-3 py-1.5 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
          >
            Revisar este doc
          </button>
          <AutonomySelector mode={autonomyMode} onChange={handleAutonomyChange} />
          <span className="t-body-sm text-[var(--color-text-tertiary)]">
            {savedAt ? `Saved ${savedAt.toLocaleTimeString()}` : "Not saved yet"}
          </span>
        </div>
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
              onProposal={handleProposal}
            />
          ) : (
            <DemoChatPlaceholder />
          )
        }
        right={
          <div className="p-4 flex flex-col gap-3">
            <Canvas initialMarkdown={STARTER_MARKDOWN} onSave={handleSave} />
            <ProposalPanel
              proposals={proposals.proposals}
              onAccept={(id) => {
                // The send happens via ChatPane's WS; to keep a single
                // source of truth, we delegate through a DOM custom
                // event that the ChatPane listens for. This sidesteps
                // prop-drilling the `sendRaw` reference into EditorV2
                // without needing a context provider yet.
                proposals.resolve(id);
                window.dispatchEvent(
                  new CustomEvent("historiador:block-op-ack", {
                    detail: { proposalId: id, decision: "accepted" },
                  })
                );
              }}
              onReject={(id) => {
                proposals.resolve(id);
                window.dispatchEvent(
                  new CustomEvent("historiador:block-op-ack", {
                    detail: { proposalId: id, decision: "rejected" },
                  })
                );
              }}
            />
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
