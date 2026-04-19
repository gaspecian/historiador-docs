/**
 * Editor-v2 WebSocket client (Sprint 11, phase A3).
 *
 * Mirrors `apps/api/src/presentation/handler/editor_ws.rs`. The client
 * opens one WebSocket per editor tab, receives a `hello` from the
 * server, replies with `client_hello` carrying its last observed
 * `seq`, then processes messages until the socket closes.
 *
 * On disconnect the hook reconnects with exponential backoff (1s, 2s,
 * 4s, 8s, capped at 30s). On reconnect it sends `client_hello` with
 * the last `seq` it has — the server replays anything newer than that.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

export type EditorMessage =
  | {
      type: "hello";
      protocol_version: string;
      supported_variants: string[];
      server_last_seq: number;
    }
  | { type: "client_hello"; client_last_seq: number }
  | { type: "message"; seq: number; role: string; content: string }
  | { type: "ack"; client_ref: string; seq: number }
  | { type: "error"; code: string; message: string }
  | {
      type: "tool_call";
      seq: number;
      call_id: string;
      name: string;
      arguments: unknown;
    }
  | {
      type: "block_op";
      seq: number;
      proposal_id: string;
      op: unknown;
    }
  | {
      type: "block_op_ack";
      proposal_id: string;
      decision: string;
    }
  | { type: "skip_discovery" }
  | {
      type: "outline_proposed";
      seq: number;
      sections: OutlineSection[];
    }
  | {
      type: "outline_revised";
      seq: number;
      sections: OutlineSection[];
    }
  | {
      type: "outline_approved";
      sections: OutlineSection[];
    }
  | {
      type: "autonomy_mode_changed";
      mode: AutonomyMode;
    }
  | {
      type: "autonomy_checkpoint";
      seq: number;
      summary: string;
      op_count: number;
      reason: "heading_boundary" | "op_threshold" | "timeout";
    }
  | {
      type: "autonomy_decision";
      decision: "continue" | "revise" | "skip";
    }
  | {
      type: "comment_posted";
      seq: number;
      comment_id: string;
      block_ids: string[];
      text: string;
    }
  | {
      type: "comment_resolved";
      seq: number;
      comment_id: string;
    };

export interface OutlineSection {
  heading: string;
  level?: number;
  bullets?: string[];
}

export type AutonomyMode = "propose" | "checkpointed" | "autonomous";

export type EditorSocketStatus = "connecting" | "open" | "closed" | "error";

export interface UseEditorSocketArgs {
  pageId: string;
  language: string;
  /** JWT passed via query string. Should be a short-lived access token. */
  token: string;
  /** Set to false to opt out of the connection (e.g., flag off). */
  enabled?: boolean;
  onMessage?: (msg: Extract<EditorMessage, { type: "message" }>) => void;
  onError?: (err: Extract<EditorMessage, { type: "error" }>) => void;
  /** Fired when the server proposes or revises an outline (A9). */
  onOutline?: (sections: OutlineSection[], revised: boolean) => void;
  /** Fired when the server pushes a block_op envelope (A10). */
  onBlockOp?: (proposalId: string, op: unknown) => void;
  /** Fired when the server pushes an autonomy_checkpoint (A11). */
  onCheckpoint?: (summary: string, opCount: number, reason: string) => void;
  /** Fired when the autonomy mode changes (either direction). */
  onAutonomyMode?: (mode: AutonomyMode) => void;
}

export interface UseEditorSocketResult {
  status: EditorSocketStatus;
  lastSeq: number;
  send: (role: string, content: string) => void;
  /** Send an arbitrary control envelope. Forward-compatible with the
   *  ADR-012 "unknown variants are dropped" rule — callers can push
   *  variants the server has not yet grown without breaking anything. */
  sendRaw: (msg: EditorMessage) => void;
  protocolVersion: string | null;
  serverSupportedVariants: string[];
}

const BACKOFF_MS = [1000, 2000, 4000, 8000, 16000, 30000];

export function useEditorSocket(args: UseEditorSocketArgs): UseEditorSocketResult {
  const {
    pageId,
    language,
    token,
    enabled = true,
    onMessage,
    onError,
    onOutline,
    onBlockOp,
    onCheckpoint,
    onAutonomyMode,
  } = args;

  const [status, setStatus] = useState<EditorSocketStatus>("closed");
  const [lastSeq, setLastSeq] = useState<number>(0);
  const [protocolVersion, setProtocolVersion] = useState<string | null>(null);
  const [serverSupportedVariants, setServerSupportedVariants] = useState<string[]>([]);

  const socketRef = useRef<WebSocket | null>(null);
  const backoffIndexRef = useRef(0);
  const reconnectTimerRef = useRef<number | null>(null);
  const lastSeqRef = useRef(0);

  useEffect(() => {
    lastSeqRef.current = lastSeq;
  }, [lastSeq]);

  const url = useMemo(() => {
    if (!enabled || !token) return null;
    // Next.js proxies `/api/*` to the Axum backend via next.config.ts.
    // For WebSockets we have to hit the backend directly — use the
    // same-origin path with `ws://`/`wss://` depending on page scheme.
    const proto = typeof window !== "undefined" && window.location.protocol === "https:" ? "wss:" : "ws:";
    const host = typeof window !== "undefined" ? window.location.host : "";
    const qs = new URLSearchParams({
      page_id: pageId,
      language,
      token,
    });
    return `${proto}//${host}/api/editor/ws?${qs.toString()}`;
  }, [enabled, pageId, language, token]);

  useEffect(() => {
    if (!url) {
      // When the hook is disabled (flag off, no token, etc.) we skip
      // the connection entirely. `status` stays at its initial
      // "closed" value; the `onclose` handler updates it to "closed"
      // again on real disconnects, so both paths converge.
      return;
    }

    let cancelled = false;

    const open = () => {
      if (cancelled) return;
      setStatus("connecting");
      const ws = new WebSocket(url);
      socketRef.current = ws;

      ws.onopen = () => {
        if (cancelled) return;
        setStatus("open");
        backoffIndexRef.current = 0;
        const hello: EditorMessage = {
          type: "client_hello",
          client_last_seq: lastSeqRef.current,
        };
        ws.send(JSON.stringify(hello));
      };

      ws.onmessage = (event) => {
        if (cancelled) return;
        let msg: EditorMessage;
        try {
          msg = JSON.parse(event.data as string) as EditorMessage;
        } catch {
          // Unknown envelope — drop per ADR-012 forward-compat rule.
          return;
        }
        switch (msg.type) {
          case "hello":
            setProtocolVersion(msg.protocol_version);
            setServerSupportedVariants(msg.supported_variants);
            // Adopt server's last_seq if higher — helps reconcile
            // cases where the client's local counter drifted.
            if (msg.server_last_seq > lastSeqRef.current) {
              setLastSeq(msg.server_last_seq);
            }
            break;
          case "message":
            if (msg.seq > lastSeqRef.current) {
              setLastSeq(msg.seq);
            }
            onMessage?.(msg);
            break;
          case "ack":
            if (msg.seq > lastSeqRef.current) {
              setLastSeq(msg.seq);
            }
            break;
          case "error":
            onError?.(msg);
            break;
          case "outline_proposed":
            if (msg.seq > lastSeqRef.current) {
              setLastSeq(msg.seq);
            }
            onOutline?.(msg.sections, false);
            break;
          case "outline_revised":
            if (msg.seq > lastSeqRef.current) {
              setLastSeq(msg.seq);
            }
            onOutline?.(msg.sections, true);
            break;
          case "block_op":
            if (msg.seq > lastSeqRef.current) {
              setLastSeq(msg.seq);
            }
            onBlockOp?.(msg.proposal_id, msg.op);
            break;
          case "autonomy_checkpoint":
            if (msg.seq > lastSeqRef.current) {
              setLastSeq(msg.seq);
            }
            onCheckpoint?.(msg.summary, msg.op_count, msg.reason);
            break;
          case "autonomy_mode_changed":
            onAutonomyMode?.(msg.mode);
            break;
          default:
            // Unknown variant — drop silently.
            break;
        }
      };

      ws.onerror = () => {
        if (cancelled) return;
        setStatus("error");
      };

      ws.onclose = () => {
        if (cancelled) return;
        setStatus("closed");
        scheduleReconnect();
      };
    };

    const scheduleReconnect = () => {
      if (cancelled) return;
      const delay = BACKOFF_MS[Math.min(backoffIndexRef.current, BACKOFF_MS.length - 1)];
      backoffIndexRef.current += 1;
      reconnectTimerRef.current = window.setTimeout(open, delay);
    };

    open();

    return () => {
      cancelled = true;
      if (reconnectTimerRef.current != null) {
        window.clearTimeout(reconnectTimerRef.current);
      }
      socketRef.current?.close();
      socketRef.current = null;
    };
  }, [url, onMessage, onError, onOutline, onBlockOp, onCheckpoint, onAutonomyMode]);

  const send = useCallback((role: string, content: string) => {
    const ws = socketRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const msg: EditorMessage = {
      type: "message",
      seq: 0, // server stamps the real seq
      role,
      content,
    };
    ws.send(JSON.stringify(msg));
  }, []);

  const sendRaw = useCallback((msg: EditorMessage) => {
    const ws = socketRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(JSON.stringify(msg));
  }, []);

  return {
    status,
    lastSeq,
    send,
    sendRaw,
    protocolVersion,
    serverSupportedVariants,
  };
}
