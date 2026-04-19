"use client";

/**
 * Autonomy mode selector (Sprint 11, phase A11 / ADR-014).
 *
 * Three modes per-page:
 *   - Propose      — every op becomes a proposal the author accepts
 *   - Checkpointed — batches of ops pause at section boundaries
 *   - Autonomous   — ops auto-apply after a short delay
 *
 * Default is Propose. The selected mode appears in the top bar so
 * the user sees it on every turn.
 */

import { AlertTriangle, ChevronDown, Hand, Zap } from "lucide-react";
import { useState } from "react";

import type { AutonomyMode } from "@/lib/editor-ws";

export interface AutonomySelectorProps {
  mode: AutonomyMode;
  onChange: (mode: AutonomyMode) => void;
  disabled?: boolean;
}

const OPTIONS: Array<{ value: AutonomyMode; label: string; hint: string }> = [
  {
    value: "propose",
    label: "Propose",
    hint: "Cada mudança vira uma proposta que você aprova ou rejeita.",
  },
  {
    value: "checkpointed",
    label: "Checkpointed",
    hint: "O agente escreve em seções e pausa para você revisar.",
  },
  {
    value: "autonomous",
    label: "Autonomous",
    hint: "Mudanças aplicam sozinhas após 1,5 s — use com cuidado.",
  },
];

export function AutonomySelector({ mode, onChange, disabled = false }: AutonomySelectorProps) {
  const [open, setOpen] = useState(false);
  const current = OPTIONS.find((o) => o.value === mode) ?? OPTIONS[0];

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        disabled={disabled}
        className="inline-flex items-center gap-2 t-body-sm rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] px-3 py-1.5 hover:bg-[var(--color-surface-hover)] disabled:opacity-60"
      >
        <Icon mode={mode} />
        {current.label}
        <ChevronDown className="w-3.5 h-3.5" aria-hidden />
      </button>
      {open && (
        <ul
          role="menu"
          className="absolute right-0 mt-1 z-10 w-[260px] rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] shadow-[var(--shadow-md)] p-1"
        >
          {OPTIONS.map((opt) => (
            <li key={opt.value}>
              <button
                type="button"
                onClick={() => {
                  onChange(opt.value);
                  setOpen(false);
                }}
                className={`w-full text-left p-2 rounded-[var(--radius-sm)] flex flex-col gap-0.5 hover:bg-[var(--color-surface-hover)] ${
                  opt.value === mode ? "bg-[var(--color-primary-50)]" : ""
                }`}
              >
                <span className="flex items-center gap-2 t-body-sm font-semibold">
                  <Icon mode={opt.value} />
                  {opt.label}
                </span>
                <span className="t-body-sm text-[var(--color-text-tertiary)]">{opt.hint}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function Icon({ mode }: { mode: AutonomyMode }) {
  switch (mode) {
    case "propose":
      return <Hand className="w-3.5 h-3.5 text-[var(--color-primary-600)]" aria-hidden />;
    case "checkpointed":
      return <AlertTriangle className="w-3.5 h-3.5 text-[var(--color-amber-600)]" aria-hidden />;
    case "autonomous":
      return <Zap className="w-3.5 h-3.5 text-[var(--color-teal-600)]" aria-hidden />;
  }
}
