"use client";

/**
 * Outline proposal card (Sprint 11, phase A9 / ADR-015).
 *
 * Shown inline in the chat stream when the server emits an
 * `outline_proposed` or `outline_revised` envelope. Clicking
 * "Approve outline" sends an `outline_approved` envelope back;
 * the server then seeds the canvas with one heading per section
 * via A10's proposal overlay.
 */

import { Check, ListChecks } from "lucide-react";

export interface OutlineSection {
  heading: string;
  level?: number;
  bullets?: string[];
}

export interface OutlineCardProps {
  sections: OutlineSection[];
  onApprove: () => void;
  approved?: boolean;
}

export function OutlineCard({ sections, onApprove, approved = false }: OutlineCardProps) {
  return (
    <div className="self-start max-w-[95%] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] rounded-[var(--radius-md)] p-3 flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <ListChecks className="w-4 h-4 text-[var(--color-primary-600)]" aria-hidden />
        <span className="t-label text-[var(--color-text-secondary)]">
          {approved ? "Outline approved" : "Proposed outline"}
        </span>
      </div>
      <ol className="flex flex-col gap-2 list-none pl-0 m-0">
        {sections.map((s, i) => (
          <li key={`${s.heading}-${i}`} className="flex flex-col">
            <div className="t-body-sm font-semibold">
              {i + 1}. {s.heading}
            </div>
            {s.bullets && s.bullets.length > 0 && (
              <ul className="t-body-sm text-[var(--color-text-secondary)] list-disc pl-5 m-0">
                {s.bullets.map((b, j) => (
                  <li key={j}>{b}</li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ol>
      {!approved && (
        <button
          type="button"
          onClick={onApprove}
          className="self-start inline-flex items-center gap-2 t-body-sm px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--color-primary-600)] text-[var(--color-text-inverse)] hover:bg-[var(--color-primary-700)]"
        >
          <Check className="w-4 h-4" aria-hidden />
          Aprovar outline
        </button>
      )}
    </div>
  );
}
