"use client";

/**
 * Inline comment composer (Sprint 11, phase B1).
 *
 * Modal-free: when the user clicks "Novo" on the panel, this
 * textarea appears at the top. ⏎ submits, Esc cancels. The
 * scope is either the currently-focused block (via the
 * `cursor_block_id` prop threaded in from the canvas) or the
 * whole page when nothing is focused.
 */

import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Send, X } from "lucide-react";

export interface CommentComposerProps {
  /** Block ID the comment anchors to; null means whole-page. */
  anchorBlockId: string | null;
  onSubmit: (blockIds: string[], text: string) => void;
  onCancel: () => void;
}

export function CommentComposer({ anchorBlockId, onSubmit, onCancel }: CommentComposerProps) {
  const [value, setValue] = useState("");
  const ref = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    ref.current?.focus();
  }, []);

  const submit = useCallback(() => {
    const text = value.trim();
    if (!text) return;
    const blockIds = anchorBlockId ? [anchorBlockId] : [];
    onSubmit(blockIds, text);
    setValue("");
  }, [anchorBlockId, onSubmit, value]);

  const onKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        submit();
      } else if (e.key === "Escape") {
        e.preventDefault();
        onCancel();
      }
    },
    [submit, onCancel]
  );

  return (
    <div className="flex flex-col gap-2 rounded-[var(--radius-md)] border border-[var(--color-primary-600)] bg-[var(--color-surface-canvas)] p-3">
      <span className="t-body-sm text-[var(--color-text-secondary)]">
        {anchorBlockId
          ? `Comentando no bloco ${anchorBlockId.slice(0, 8)}…`
          : "Comentário sobre a página inteira"}
      </span>
      <textarea
        ref={ref}
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={onKeyDown}
        rows={2}
        placeholder="Escreva sua observação…"
        className="resize-none rounded-[var(--radius-sm)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] px-2 py-1 t-body-sm focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-600)]"
      />
      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          aria-label="Cancelar"
          className="inline-flex items-center gap-1 t-body-sm px-2 py-1 rounded-[var(--radius-sm)] border border-[var(--color-surface-border)] hover:bg-[var(--color-surface-hover)]"
        >
          <X className="w-3.5 h-3.5" aria-hidden />
          Cancelar
        </button>
        <button
          type="button"
          onClick={submit}
          disabled={value.trim().length === 0}
          className="inline-flex items-center gap-1 t-body-sm px-3 py-1 rounded-[var(--radius-sm)] bg-[var(--color-primary-600)] text-[var(--color-text-inverse)] disabled:opacity-40 hover:bg-[var(--color-primary-700)]"
        >
          <Send className="w-3.5 h-3.5" aria-hidden />
          Enviar
        </button>
      </div>
    </div>
  );
}
