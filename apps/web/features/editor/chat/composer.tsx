"use client";

import { useCallback, useState, type KeyboardEvent } from "react";
import { Send } from "lucide-react";

export interface ComposerProps {
  onSubmit: (content: string) => void;
  disabled?: boolean;
}

export function Composer({ onSubmit, disabled = false }: ComposerProps) {
  const [value, setValue] = useState("");

  const submit = useCallback(() => {
    const text = value.trim();
    if (!text) return;
    onSubmit(text);
    setValue("");
  }, [onSubmit, value]);

  const onKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        submit();
      }
    },
    [submit]
  );

  return (
    <div className="border-t border-[var(--color-surface-border)] p-3 bg-[var(--color-surface-page)]">
      <div className="flex gap-2 items-end">
        <textarea
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={onKeyDown}
          rows={2}
          disabled={disabled}
          placeholder="Escreva para o assistente…"
          className="flex-1 resize-none rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] px-3 py-2 t-body-sm focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-600)] disabled:opacity-60"
        />
        <button
          type="button"
          onClick={submit}
          disabled={disabled || value.trim().length === 0}
          aria-label="Send"
          className="h-10 w-10 rounded-[var(--radius-md)] bg-[var(--color-primary-600)] text-[var(--color-text-inverse)] flex items-center justify-center disabled:opacity-40 disabled:cursor-not-allowed hover:bg-[var(--color-primary-700)]"
        >
          <Send className="w-4 h-4" aria-hidden />
        </button>
      </div>
    </div>
  );
}
