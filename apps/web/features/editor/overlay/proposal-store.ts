/**
 * Proposal overlay state (Sprint 11, phase A10 / ADR-013).
 *
 * The canvas has two layers:
 *   - **base**: the persisted markdown, auto-saved every 30 s
 *   - **overlay**: pending proposals the user must accept or reject
 *
 * The overlay is pure React state — it never auto-saves. That is
 * the mechanical enforcement of the "unapproved AI content never
 * auto-saves" invariant (ADR-013 §24–31). When the user accepts a
 * proposal we move it into the base + emit `block_op_ack` so the
 * server can audit and the next `hello` replay is consistent.
 */

import { useCallback, useState } from "react";

export type ProposalKind = "insert" | "replace" | "append" | "delete" | "suggest";

export interface Proposal {
  proposalId: string;
  kind: ProposalKind;
  /** Human-readable summary rendered in the overlay card. */
  summary: string;
  /** Target block ID, if any (absent for append with multiple blocks). */
  blockId?: string;
  /** Full op payload as received from the server; opaque to the UI. */
  raw: unknown;
}

export interface ProposalStore {
  proposals: Proposal[];
  add: (proposal: Proposal) => void;
  resolve: (proposalId: string) => Proposal | undefined;
  clear: () => void;
}

export function useProposalStore(): ProposalStore {
  const [proposals, setProposals] = useState<Proposal[]>([]);

  const add = useCallback((p: Proposal) => {
    setProposals((prev) => {
      if (prev.some((x) => x.proposalId === p.proposalId)) return prev;
      return [...prev, p];
    });
  }, []);

  const resolve = useCallback(
    (proposalId: string) => {
      let removed: Proposal | undefined;
      setProposals((prev) => {
        removed = prev.find((p) => p.proposalId === proposalId);
        return prev.filter((p) => p.proposalId !== proposalId);
      });
      return removed;
    },
    []
  );

  const clear = useCallback(() => setProposals([]), []);

  return { proposals, add, resolve, clear };
}

/**
 * Best-effort extraction of a human summary from the raw block_op
 * payload. The server emits proposals as
 * `{ proposal_id, op: { kind, ...details } }` envelopes. This does
 * not need to be perfect — the overlay card shows it in muted text
 * next to Accept/Reject controls.
 */
export function summariseOp(op: unknown): { summary: string; kind: ProposalKind } {
  const typed = op as { kind?: string; block?: { text?: string; heading?: string } } | null;
  const kind = (typed?.kind ?? "insert") as ProposalKind;
  const text = typed?.block?.text ?? typed?.block?.heading ?? "";
  const trimmed = text.length > 80 ? `${text.slice(0, 77)}…` : text;
  const label = trimmed.length > 0 ? `: "${trimmed}"` : "";
  const verb: Record<ProposalKind, string> = {
    insert: "Inserir bloco",
    replace: "Substituir bloco",
    append: "Adicionar à seção",
    delete: "Remover bloco",
    suggest: "Sugerir mudança",
  };
  return { kind, summary: `${verb[kind] ?? "Editar"}${label}` };
}
