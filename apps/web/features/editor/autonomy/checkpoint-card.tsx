"use client";

/**
 * Checkpoint card (Sprint 11, phase A11 / US-11.08).
 *
 * Rendered when the server emits an `autonomy_checkpoint` envelope.
 * Shows the section the agent just finished and three controls:
 *   - Continuar escrevendo
 *   - Refinar esta seção
 *   - Pular
 *
 * Copy follows the design-system voice guide (Portuguese, sentence
 * case) per design-system.md §15–31.
 */

import { ArrowRight, Pencil, SkipForward } from "lucide-react";

export interface CheckpointCardProps {
  summary: string;
  opCount: number;
  reason: string;
  onContinue: () => void;
  onRevise: () => void;
  onSkip: () => void;
}

export function CheckpointCard({
  summary,
  opCount,
  reason,
  onContinue,
  onRevise,
  onSkip,
}: CheckpointCardProps) {
  return (
    <div className="self-start max-w-[95%] border-l-4 border-[var(--color-teal-500)] bg-[var(--color-teal-50)] rounded-[var(--radius-md)] p-3 flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <span className="t-label text-[var(--color-teal-700)]">Checkpoint</span>
        <span className="t-body-sm text-[var(--color-text-tertiary)]">
          {opCount} mudança{opCount === 1 ? "" : "s"} · {reasonLabel(reason)}
        </span>
      </div>
      <p className="t-body-sm text-[var(--color-text-primary)] whitespace-pre-wrap break-words">
        {summary}
      </p>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={onContinue}
          className="inline-flex items-center gap-2 t-body-sm px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--color-primary-600)] text-[var(--color-text-inverse)] hover:bg-[var(--color-primary-700)]"
        >
          <ArrowRight className="w-4 h-4" aria-hidden />
          Continuar escrevendo
        </button>
        <button
          type="button"
          onClick={onRevise}
          className="inline-flex items-center gap-2 t-body-sm px-3 py-1.5 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] hover:bg-[var(--color-surface-hover)]"
        >
          <Pencil className="w-4 h-4" aria-hidden />
          Refinar esta seção
        </button>
        <button
          type="button"
          onClick={onSkip}
          className="inline-flex items-center gap-2 t-body-sm px-3 py-1.5 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)]"
        >
          <SkipForward className="w-4 h-4" aria-hidden />
          Pular
        </button>
      </div>
    </div>
  );
}

function reasonLabel(reason: string): string {
  switch (reason) {
    case "heading_boundary":
      return "fim de seção";
    case "op_threshold":
      return "limite de mudanças";
    case "timeout":
      return "pausa automática";
    default:
      return reason;
  }
}
