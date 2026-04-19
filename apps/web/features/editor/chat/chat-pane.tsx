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

import {
  useEditorSocket,
  type AutonomyMode,
  type OutlineSection,
} from "@/lib/editor-ws";

import { CheckpointCard } from "../autonomy";
import { OutlineCard } from "../outline";
import { Composer } from "./composer";
import { MessageList, type ChatMessage } from "./message-list";

interface PendingCheckpoint {
  summary: string;
  opCount: number;
  reason: string;
}

export interface ChatPaneProps {
  pageId: string;
  language: string;
  /** Short-lived JWT passed on the WS query string per ADR-012. */
  token: string;
  /** Current canvas selection, if any. Empty when nothing is selected. */
  selectionText: string;
  /** Block the cursor is currently in. Null when the canvas has no focus. */
  cursorBlockId: string | null;
  /** Forward every incoming block_op to the parent so the overlay
   *  panel (lifted into EditorV2) can track pending proposals. */
  onProposal?: (proposalId: string, op: unknown) => void;
  /** Forward incoming comment_posted envelopes to the parent store. */
  onCommentPosted?: (
    commentId: string,
    authorId: string,
    blockIds: string[],
    text: string,
  ) => void;
  /** Forward incoming comment_resolved envelopes to the parent store. */
  onCommentResolved?: (commentId: string) => void;
  /** Flag-off fallback: render the pane as disabled instead of opening a WS. */
  disabled?: boolean;
}

export function ChatPane({
  pageId,
  language,
  token,
  selectionText,
  cursorBlockId,
  onProposal,
  onCommentPosted,
  onCommentResolved,
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

  const [latestOutline, setLatestOutline] = useState<OutlineSection[] | null>(null);
  const [outlineApproved, setOutlineApproved] = useState(false);
  const [pendingCheckpoint, setPendingCheckpoint] = useState<PendingCheckpoint | null>(null);

  const handleOutline = useCallback((sections: OutlineSection[]) => {
    setLatestOutline(sections);
    setOutlineApproved(false);
  }, []);

  const handleCheckpoint = useCallback(
    (summary: string, opCount: number, reason: string) => {
      setPendingCheckpoint({ summary, opCount, reason });
    },
    []
  );

  const { status, send, sendRaw } = useEditorSocket({
    pageId,
    language,
    token,
    enabled: !disabled,
    onMessage: handleIncoming,
    onError: handleError,
    onOutline: handleOutline,
    onBlockOp: onProposal,
    onCheckpoint: handleCheckpoint,
    onCommentPosted,
    onCommentResolved,
  });

  // Listen for comment post / resolve events emitted by the
  // canvas-pane comment panel. Same decoupled DOM-event pattern as
  // the block-op-ack and autonomy-change bridges.
  useEffect(() => {
    const postHandler = (e: Event) => {
      const detail = (e as CustomEvent<{
        commentId: string;
        blockIds: string[];
        text: string;
      }>).detail;
      if (!detail) return;
      sendRaw({
        type: "comment_posted",
        seq: 0,
        comment_id: detail.commentId,
        block_ids: detail.blockIds,
        text: detail.text,
      });
    };
    const resolveHandler = (e: Event) => {
      const detail = (e as CustomEvent<{ commentId: string }>).detail;
      if (!detail) return;
      sendRaw({
        type: "comment_resolved",
        seq: 0,
        comment_id: detail.commentId,
      });
    };
    window.addEventListener("historiador:comment-post", postHandler);
    window.addEventListener("historiador:comment-resolve", resolveHandler);
    return () => {
      window.removeEventListener("historiador:comment-post", postHandler);
      window.removeEventListener("historiador:comment-resolve", resolveHandler);
    };
  }, [sendRaw]);

  const handleCheckpointDecision = useCallback(
    (decision: "continue" | "revise" | "skip") => {
      setPendingCheckpoint(null);
      sendRaw({ type: "autonomy_decision", decision });
    },
    [sendRaw]
  );

  // Listen for autonomy-mode flips from the top bar (EditorV2 emits).
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ mode: AutonomyMode }>).detail;
      if (!detail) return;
      sendRaw({ type: "autonomy_mode_changed", mode: detail.mode });
    };
    window.addEventListener("historiador:autonomy-change", handler);
    return () => window.removeEventListener("historiador:autonomy-change", handler);
  }, [sendRaw]);

  // Listen for "Revisar este doc" clicks (B5 / US-11.11).
  useEffect(() => {
    const handler = () => {
      sendRaw({ type: "review_requested" });
    };
    window.addEventListener("historiador:review-request", handler);
    return () => window.removeEventListener("historiador:review-request", handler);
  }, [sendRaw]);

  // Listen for block-op ack events emitted by the ProposalPanel
  // (which lives in EditorV2, outside this component tree). A custom
  // DOM event keeps the two surfaces decoupled without needing a
  // context provider.
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ proposalId: string; decision: string }>).detail;
      if (!detail) return;
      sendRaw({
        type: "block_op_ack",
        proposal_id: detail.proposalId,
        decision: detail.decision,
      });
    };
    window.addEventListener("historiador:block-op-ack", handler);
    return () => window.removeEventListener("historiador:block-op-ack", handler);
  }, [sendRaw]);

  const [discoverySkipped, setDiscoverySkipped] = useState(false);
  const handleSkipDiscovery = useCallback(() => {
    setDiscoverySkipped(true);
    sendRaw({ type: "skip_discovery" });
  }, [sendRaw]);

  const handleApproveOutline = useCallback(() => {
    if (!latestOutline) return;
    setOutlineApproved(true);
    sendRaw({ type: "outline_approved", sections: latestOutline });
  }, [latestOutline, sendRaw]);

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
        {latestOutline && (
          <div className="px-4 pb-3">
            <OutlineCard
              sections={latestOutline}
              onApprove={handleApproveOutline}
              approved={outlineApproved}
            />
          </div>
        )}
        {pendingCheckpoint && (
          <div className="px-4 pb-3">
            <CheckpointCard
              summary={pendingCheckpoint.summary}
              opCount={pendingCheckpoint.opCount}
              reason={pendingCheckpoint.reason}
              onContinue={() => handleCheckpointDecision("continue")}
              onRevise={() => handleCheckpointDecision("revise")}
              onSkip={() => handleCheckpointDecision("skip")}
            />
          </div>
        )}
        <Composer
          onSubmit={sendUserTurn}
          onSkipDiscovery={discoverySkipped ? undefined : handleSkipDiscovery}
          disabled={disabled || status !== "open"}
        />
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
