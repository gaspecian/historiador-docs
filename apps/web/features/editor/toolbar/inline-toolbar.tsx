"use client";

/**
 * Inline selection toolbar (Sprint 11, phase B2 / US-11.10).
 *
 * Appears as a floating bar when the user selects text in the
 * canvas. Each button dispatches a pre-formatted instruction to
 * the chat (via the same `historiador:canvas-instruction` DOM
 * event pattern the overlay uses). The chat picks it up, ships a
 * user turn to the WS, and the agent replies with the appropriate
 * tool call anchored to the selection's block.
 */

import {
  MessageCircleQuestion,
  PencilRuler,
  Scissors,
  Sparkles,
  SpellCheck2,
} from "lucide-react";

export type InlineAction = "rewrite" | "expand" | "shorten" | "fix_grammar" | "ask";

export interface InlineToolbarProps {
  /** The currently selected text, if any. Toolbar hides when empty. */
  selectionText: string;
  /** Screen-space anchor for positioning. */
  anchor?: { top: number; left: number };
  onAction: (action: InlineAction, selection: string) => void;
}

const ACTIONS: Array<{
  id: InlineAction;
  label: string;
  Icon: typeof Sparkles;
}> = [
  { id: "rewrite", label: "Reescrever", Icon: PencilRuler },
  { id: "expand", label: "Expandir", Icon: Sparkles },
  { id: "shorten", label: "Encurtar", Icon: Scissors },
  { id: "fix_grammar", label: "Corrigir gramática", Icon: SpellCheck2 },
  { id: "ask", label: "Perguntar", Icon: MessageCircleQuestion },
];

export function InlineToolbar({ selectionText, anchor, onAction }: InlineToolbarProps) {
  if (!selectionText.trim()) return null;

  const style: React.CSSProperties = anchor
    ? { position: "absolute", top: anchor.top, left: anchor.left, zIndex: 50 }
    : { position: "fixed", bottom: 96, left: "50%", transform: "translateX(-50%)", zIndex: 50 };

  return (
    <div
      role="toolbar"
      aria-label="AI quick actions"
      style={style}
      className="flex items-center gap-1 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] shadow-[var(--shadow-md)] px-1 py-1"
    >
      {ACTIONS.map(({ id, label, Icon }) => (
        <button
          key={id}
          type="button"
          onClick={() => onAction(id, selectionText)}
          aria-label={label}
          className="inline-flex items-center gap-1 t-body-sm px-2 py-1 rounded-[var(--radius-sm)] hover:bg-[var(--color-surface-hover)]"
        >
          <Icon className="w-3.5 h-3.5" aria-hidden />
          {label}
        </button>
      ))}
    </div>
  );
}

/**
 * Helper: turn an InlineAction + selection into the user-facing text
 * the chat ships to the agent. Keeps the copy centralised so the
 * agent sees a consistent phrasing across surfaces.
 */
export function formatInstruction(action: InlineAction, selection: string): string {
  const quoted = selection.trim();
  switch (action) {
    case "rewrite":
      return `Reescreva este trecho mantendo o significado mas com mais clareza:\n\n"${quoted}"`;
    case "expand":
      return `Expanda este trecho com mais detalhes:\n\n"${quoted}"`;
    case "shorten":
      return `Resuma este trecho de forma mais concisa:\n\n"${quoted}"`;
    case "fix_grammar":
      return `Corrija a gramática e a fluência deste trecho, preservando o significado:\n\n"${quoted}"`;
    case "ask":
      return `Sobre este trecho, tenho uma pergunta:\n\n"${quoted}"\n\n`;
  }
}
