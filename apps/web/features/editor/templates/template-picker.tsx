"use client";

/**
 * Template picker (Sprint 11, phase B3 / US-11.03).
 *
 * Shown on a blank canvas. Selecting a template seeds the canvas
 * with its markdown — routed through the A10 overlay in Propose
 * mode so the user can still reject before the content lands.
 */

import { BookOpen, Code2, GraduationCap, ListChecks, Siren } from "lucide-react";

export type TemplateId = "runbook" | "api-doc" | "onboarding" | "post-mortem" | "tutorial";

export interface TemplatePickerProps {
  onPick: (id: TemplateId) => void;
}

const TEMPLATES: Array<{
  id: TemplateId;
  label: string;
  hint: string;
  Icon: typeof BookOpen;
}> = [
  {
    id: "runbook",
    label: "Runbook",
    hint: "Playbook para incidentes e operações.",
    Icon: Siren,
  },
  {
    id: "api-doc",
    label: "API",
    hint: "Referência de endpoint com parâmetros e exemplos.",
    Icon: Code2,
  },
  {
    id: "onboarding",
    label: "Onboarding",
    hint: "Primeira semana de uma pessoa nova no time.",
    Icon: GraduationCap,
  },
  {
    id: "post-mortem",
    label: "Post-mortem",
    hint: "Relatório de incidente com timeline e ações.",
    Icon: ListChecks,
  },
  {
    id: "tutorial",
    label: "Tutorial",
    hint: "Passo a passo construindo algo concreto.",
    Icon: BookOpen,
  },
];

export function TemplatePicker({ onPick }: TemplatePickerProps) {
  return (
    <div className="flex flex-col gap-3 p-6 rounded-[var(--radius-lg)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)]">
      <h2 className="t-h3">Começar com um template</h2>
      <p className="t-body-sm text-[var(--color-text-secondary)]">
        Escolha um ponto de partida ou feche e escreva do zero. Você ainda
        aceita ou rejeita cada bloco que o template insere.
      </p>
      <ul className="grid grid-cols-1 sm:grid-cols-2 gap-2 list-none pl-0 m-0">
        {TEMPLATES.map(({ id, label, hint, Icon }) => (
          <li key={id}>
            <button
              type="button"
              onClick={() => onPick(id)}
              className="w-full text-left p-3 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] hover:bg-[var(--color-surface-hover)] flex gap-3 items-start"
            >
              <Icon className="w-5 h-5 mt-0.5 text-[var(--color-primary-600)]" aria-hidden />
              <div className="flex flex-col">
                <span className="t-body-sm font-semibold">{label}</span>
                <span className="t-body-sm text-[var(--color-text-tertiary)]">{hint}</span>
              </div>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
