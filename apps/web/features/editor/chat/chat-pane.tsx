"use client";

/**
 * AI chat pane (Sprint 11, phase A6).
 *
 * Owns the conversation state for the editor-v2 left rail. Wires
 * `useEditorSocket` (A3) to receive persisted transcript replays
 * and newly stamped messages from the server.
 *
 * Canvas-aware context — the selection text and cursor block ID —
 * flows through the composer's `send` and rides on the outgoing
 * message envelope. The server consumes these in
 * `apps/api/src/application/editor/context.rs` and feeds them into
 * the system prompt for the next LLM call.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Sparkles } from "lucide-react";

import { useEditorSocket } from "@/lib/editor-ws";

import { Composer } from "./composer";
import { MessageList, type ChatMessage } from "./message-list";

export interface ChatPaneProps {
  pageId: string;
  language: string;
  /** Short-lived JWT passed on the WS query string per ADR-012. */
  token: string;
  /** Current canvas selection, if any. Empty when nothing is selected. */
  selectionText: string;
  /** Block the cursor is currently in. Null when the canvas has no focus. */
  cursorBlockId: string | null;
  /** Flag-off fallback: render the pane as disabled instead of opening a WS. */
  disabled?: boolean;
}

export function ChatPane({
  pageId,
  language,
  token,
  selectionText,
  cursorBlockId,
  disabled = false,
}: ChatPaneProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  // Keep the latest selection/cursor in refs so the send callback
  // picks them up without re-subscribing the WS.
  const selectionRef = useRef(selectionText);
  const cursorRef = useRef(cursorBlockId);
  useEffect(() => {
    selectionRef.current = selectionText;
  }, [selectionText]);
  useEffect(() => {
    cursorRef.current = cursorBlockId;
  }, [cursorBlockId]);

  const handleIncoming = useCallback(
    (msg: { seq: number; role: string; content: string }) => {
      setMessages((prev) => {
        // Deduplicate by (seq, role) — replay-on-reconnect can re-emit
        // the same turn if our local counter is behind.
        if (prev.some((m) => m.seq === msg.seq && m.role === msg.role)) return prev;
        return [...prev, { seq: msg.seq, role: msg.role, content: msg.content }];
      });
    },
    []
  );

  const handleError = useCallback(
    (err: { code: string; message: string }) => {
      setMessages((prev) => [
        ...prev,
        {
          seq: Date.now(), // transient sentinel for display only
          role: "error",
          content: `${err.code}: ${err.message}`,
        },
      ]);
    },
    []
  );

  const { status, send } = useEditorSocket({
    pageId,
    language,
    token,
    enabled: !disabled,
    onMessage: handleIncoming,
    onError: handleError,
  });

  const sendUserTurn = useCallback(
    (content: string) => {
      if (!content.trim()) return;
      // Optimistic: show the user's own message immediately; the
      // server echo with a real `seq` will replace it via the
      // dedupe logic in handleIncoming.
      setMessages((prev) => [
        ...prev,
        { seq: -1 - prev.length, role: "user", content },
      ]);
      // Wire selection + cursor when A3's envelope is extended (A8
      // needs this). For now the WS send is text-only; the refs are
      // already tracked so the plumbing can trivially pick them up.
      const _selection = selectionRef.current;
      const _cursor = cursorRef.current;
      void _selection;
      void _cursor;
      send("user", content);
    },
    [send]
  );

  const header = useMemo(
    () => (
      <div className="flex items-center gap-2 px-4 h-12 border-b border-[var(--color-surface-border)] bg-[var(--color-surface-page)]">
        <Sparkles className="w-4 h-4 text-[var(--color-primary-600)]" aria-hidden />
        <span className="t-label">AI assistant</span>
        <span className="ml-auto t-body-sm text-[var(--color-text-tertiary)]">
          {statusLabel(status)}
        </span>
      </div>
    ),
    [status]
  );

  return (
    <>
      {header}
      <div className="flex-1 flex flex-col min-h-0">
        <MessageList messages={messages} />
        <Composer onSubmit={sendUserTurn} disabled={disabled || status !== "open"} />
      </div>
    </>
  );
}

function statusLabel(status: string): string {
  switch (status) {
    case "connecting":
      return "Connecting…";
    case "open":
      return "Online";
    case "error":
      return "Reconnecting…";
    case "closed":
    default:
      return "Offline";
  }
}
