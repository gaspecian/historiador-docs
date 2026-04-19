"use client";

/**
 * Proposal overlay panel (Sprint 11, phase A10).
 *
 * Renders the active proposals as a stack of cards with Accept /
 * Reject controls. Keyboard shortcuts (⏎ to accept, Esc to reject)
 * operate on the top of the stack so the user can resolve without
 * leaving the keyboard.
 */

import { Check, X } from "lucide-react";
import { useEffect } from "react";

import type { Proposal, ProposalKind } from "./proposal-store";

export interface ProposalPanelProps {
  proposals: Proposal[];
  onAccept: (proposalId: string) => void;
  onReject: (proposalId: string) => void;
}

const KIND_ACCENT: Record<ProposalKind, string> = {
  insert: "bg-[var(--color-teal-50)] border-[var(--color-teal-500)]",
  replace: "bg-[var(--color-primary-50)] border-[var(--color-primary-600)]",
  append: "bg-[var(--color-teal-50)] border-[var(--color-teal-500)]",
  delete: "bg-[var(--color-red-50)] border-[var(--color-red-500)]",
  suggest: "bg-[var(--color-amber-50)] border-[var(--color-amber-500)]",
};

export function ProposalPanel({ proposals, onAccept, onReject }: ProposalPanelProps) {
  // Keyboard shortcuts: operate on the topmost pending proposal.
  useEffect(() => {
    if (proposals.length === 0) return;
    const topId = proposals[0].proposalId;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        onAccept(topId);
      } else if (e.key === "Escape") {
        e.preventDefault();
        onReject(topId);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [proposals, onAccept, onReject]);

  if (proposals.length === 0) return null;

  return (
    <aside
      aria-label="AI proposals"
      className="flex flex-col gap-2 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-subtle)] p-3"
    >
      <div className="flex items-center justify-between">
        <span className="t-label text-[var(--color-text-secondary)]">
          AI proposals ({proposals.length})
        </span>
        <span className="t-body-sm text-[var(--color-text-tertiary)]">
          ⌘↵ aceita · Esc rejeita
        </span>
      </div>
      {proposals.map((p) => (
        <div
          key={p.proposalId}
          className={`rounded-[var(--radius-md)] border-l-4 bg-[var(--color-surface-canvas)] p-2 flex items-start gap-2 ${KIND_ACCENT[p.kind]}`}
        >
          <div className="flex-1 min-w-0">
            <p className="t-body-sm text-[var(--color-text-primary)] whitespace-pre-wrap break-words">
              {p.summary}
            </p>
            {p.rationale && (
              <p className="t-body-sm italic text-[var(--color-text-secondary)] mt-1">
                “{p.rationale}”
              </p>
            )}
            {p.blockId && (
              <p className="t-body-sm text-[var(--color-text-tertiary)] font-mono text-[11px] mt-1">
                {p.blockId}
              </p>
            )}
          </div>
          <div className="flex items-center gap-1 shrink-0">
            <button
              type="button"
              onClick={() => onAccept(p.proposalId)}
              aria-label="Accept"
              className="h-8 w-8 rounded-[var(--radius-md)] bg-[var(--color-teal-600)] text-[var(--color-text-inverse)] flex items-center justify-center hover:bg-[var(--color-teal-700)]"
            >
              <Check className="w-4 h-4" aria-hidden />
            </button>
            <button
              type="button"
              onClick={() => onReject(p.proposalId)}
              aria-label="Reject"
              className="h-8 w-8 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] text-[var(--color-text-primary)] flex items-center justify-center hover:bg-[var(--color-surface-hover)]"
            >
              <X className="w-4 h-4" aria-hidden />
            </button>
          </div>
        </div>
      ))}
    </aside>
  );
}
