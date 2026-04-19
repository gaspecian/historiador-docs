"use client";

/**
 * Comment panel (Sprint 11, phase B1 / ADR-016).
 *
 * Shows every open comment on the page with a resolve button.
 * Sits under the canvas like the proposal panel; Tiptap-native
 * inline marks + margin markers are a later enhancement.
 */

import { Check, MessageSquare } from "lucide-react";

import type { Comment } from "./comment-store";

export interface CommentPanelProps {
  comments: Comment[];
  onResolve: (commentId: string) => void;
  onNew: () => void;
  /** Disabled when the WS is not open. */
  disabled?: boolean;
}

export function CommentPanel({ comments, onResolve, onNew, disabled = false }: CommentPanelProps) {
  return (
    <aside
      aria-label="Comentários"
      className="flex flex-col gap-2 rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-subtle)] p-3"
    >
      <div className="flex items-center justify-between">
        <span className="t-label text-[var(--color-text-secondary)]">
          Comentários ({comments.length})
        </span>
        <button
          type="button"
          onClick={onNew}
          disabled={disabled}
          className="inline-flex items-center gap-1 t-body-sm px-2 py-1 rounded-[var(--radius-sm)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] hover:bg-[var(--color-surface-hover)] disabled:opacity-50"
        >
          <MessageSquare className="w-3.5 h-3.5" aria-hidden />
          Novo
        </button>
      </div>
      {comments.length === 0 ? (
        <p className="t-body-sm text-[var(--color-text-tertiary)]">
          Nenhum comentário ainda. Use Novo para anotar uma dúvida ou sugestão.
        </p>
      ) : (
        <ul className="flex flex-col gap-2 list-none pl-0 m-0">
          {comments.map((c) => (
            <li
              key={c.commentId}
              className="rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] p-2 flex items-start gap-2"
            >
              <div className="flex-1 min-w-0">
                <p className="t-body-sm text-[var(--color-text-primary)] whitespace-pre-wrap break-words">
                  {c.text}
                </p>
                {c.blockIds.length > 0 && (
                  <p className="t-body-sm text-[var(--color-text-tertiary)] font-mono text-[11px] mt-1">
                    {c.blockIds.join(", ")}
                  </p>
                )}
              </div>
              <button
                type="button"
                onClick={() => onResolve(c.commentId)}
                aria-label="Resolver"
                className="h-8 w-8 rounded-[var(--radius-md)] bg-[var(--color-teal-600)] text-[var(--color-text-inverse)] flex items-center justify-center hover:bg-[var(--color-teal-700)]"
              >
                <Check className="w-4 h-4" aria-hidden />
              </button>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}
