"use client";

/**
 * Inline commentable markdown preview (Sprint 11).
 *
 * Splits the draft into top-level blocks, renders each through
 * `marked`, and wraps each in a hover surface. A block's "+" handle
 * appears on hover in the left gutter — click opens an inline
 * composer; submitting the comment calls back into the parent with
 * the block's source + the comment text.
 *
 * The parent feeds the comment to the AI via the existing
 * `iterate_draft` flow. The AI then either updates the draft
 * (new markdown arrives), replies in chat, or both — same round
 * trip the user is already familiar with.
 */

import { marked } from "marked";
import { MessageSquarePlus, CheckCircle2 } from "lucide-react";
import { useCallback, useMemo, useState } from "react";

export interface BlockComment {
  id: string;
  text: string;
  status: "pending" | "replied" | "resolved";
  replyPreview?: string;
}

export interface CommentableBlock {
  /** The raw markdown for this block (first 60 chars used as anchor snippet). */
  source: string;
  /** Pre-rendered HTML so we keep rendering consistent across parents. */
  html: string;
  /** Comments attached to this block, newest last. */
  comments: BlockComment[];
}

export interface CommentablePreviewProps {
  /** Full draft markdown. Split on blank lines into top-level blocks. */
  markdown: string;
  /** Per-block-index comment lists. Parent owns the state. */
  commentsByBlock: Record<number, BlockComment[]>;
  /** Fired when the user submits a comment on a block.
   *  `startLine` / `endLine` are 1-based line numbers into the full
   *  draft markdown so the AI can pinpoint exactly which lines are
   *  under review. */
  onComment: (
    blockIndex: number,
    blockSource: string,
    startLine: number,
    endLine: number,
    text: string,
  ) => void;
  /** Fired when the user resolves an existing comment. */
  onResolve: (blockIndex: number, commentId: string) => void;
  /** Suspend comment posting while the AI is processing the previous turn. */
  submitting?: boolean;
}

export function CommentablePreview({
  markdown,
  commentsByBlock,
  onComment,
  onResolve,
  submitting = false,
}: CommentablePreviewProps) {
  const blocks = useMemo(() => splitBlocksWithLines(markdown), [markdown]);

  return (
    <div className="md-prose flex flex-col">
      {blocks.map((block, index) => (
        <CommentableBlockWrapper
          key={`${index}-${block.source.slice(0, 20)}`}
          blockIndex={index}
          source={block.source}
          startLine={block.startLine}
          endLine={block.endLine}
          comments={commentsByBlock[index] ?? []}
          onComment={onComment}
          onResolve={onResolve}
          disabled={submitting}
        />
      ))}
    </div>
  );
}

function CommentableBlockWrapper({
  blockIndex,
  source,
  startLine,
  endLine,
  comments,
  onComment,
  onResolve,
  disabled,
}: {
  blockIndex: number;
  source: string;
  startLine: number;
  endLine: number;
  comments: BlockComment[];
  onComment: CommentablePreviewProps["onComment"];
  onResolve: CommentablePreviewProps["onResolve"];
  disabled: boolean;
}) {
  const [composerOpen, setComposerOpen] = useState(false);
  const [value, setValue] = useState("");

  const html = useMemo(() => {
    const cleaned = source.replace(/<!--\s*block:[0-9a-fA-F-]+\s*-->/g, "");
    const out = marked.parse(cleaned, { async: false, gfm: true });
    return typeof out === "string" ? out : "";
  }, [source]);

  const submit = useCallback(() => {
    const text = value.trim();
    if (!text) return;
    onComment(blockIndex, source, startLine, endLine, text);
    setValue("");
    setComposerOpen(false);
  }, [blockIndex, onComment, source, startLine, endLine, value]);

  const lineLabel =
    startLine === endLine ? `Linha ${startLine}` : `Linhas ${startLine}–${endLine}`;

  const openComments = comments.filter((c) => c.status !== "resolved");

  return (
    <div className="group relative">
      <button
        type="button"
        onClick={() => setComposerOpen((v) => !v)}
        disabled={disabled}
        aria-label="Comentar este bloco"
        className="absolute -left-10 top-1 opacity-0 group-hover:opacity-100 transition-opacity inline-flex items-center justify-center h-7 w-7 rounded-full bg-[var(--color-primary-600)] text-[var(--color-text-inverse)] hover:bg-[var(--color-primary-700)] disabled:opacity-30"
      >
        <MessageSquarePlus className="w-3.5 h-3.5" aria-hidden />
      </button>

      <div
        className={
          openComments.length > 0
            ? "border-l-2 border-[var(--color-primary-600)] pl-3 -ml-3"
            : undefined
        }
        dangerouslySetInnerHTML={{ __html: html }}
      />

      {(composerOpen || openComments.length > 0) && (
        <div className="mt-2 mb-3 flex flex-col gap-2">
          {openComments.map((c) => (
            <CommentCard
              key={c.id}
              comment={c}
              onResolve={() => onResolve(blockIndex, c.id)}
            />
          ))}
          {composerOpen && (
            <div className="rounded-[var(--radius-md)] border border-[var(--color-primary-600)] bg-[var(--color-surface-canvas)] p-3 flex flex-col gap-2">
              <span className="t-body-sm text-[var(--color-text-secondary)]">
                Comentando {lineLabel}
              </span>
              <textarea
                autoFocus
                value={value}
                onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.shiftKey) {
                    e.preventDefault();
                    submit();
                  } else if (e.key === "Escape") {
                    e.preventDefault();
                    setComposerOpen(false);
                    setValue("");
                  }
                }}
                rows={2}
                placeholder="Explique o que mudar ou pergunte à IA…"
                className="resize-none rounded-[var(--radius-sm)] border border-[var(--color-surface-border)] bg-[var(--color-surface-canvas)] px-2 py-1 t-body-sm focus:outline-none focus:ring-2 focus:ring-[var(--color-primary-600)]"
                disabled={disabled}
              />
              <div className="flex justify-between items-center">
                <span className="t-body-sm text-[var(--color-text-tertiary)]">
                  ⏎ envia à IA · Esc cancela
                </span>
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={() => {
                      setComposerOpen(false);
                      setValue("");
                    }}
                    className="t-body-sm px-2 py-1 rounded-[var(--radius-sm)] border border-[var(--color-surface-border)] hover:bg-[var(--color-surface-hover)]"
                  >
                    Cancelar
                  </button>
                  <button
                    type="button"
                    onClick={submit}
                    disabled={disabled || value.trim().length === 0}
                    className="t-body-sm px-3 py-1 rounded-[var(--radius-sm)] bg-[var(--color-primary-600)] text-[var(--color-text-inverse)] disabled:opacity-40 hover:bg-[var(--color-primary-700)]"
                  >
                    Enviar à IA
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function CommentCard({
  comment,
  onResolve,
}: {
  comment: BlockComment;
  onResolve: () => void;
}) {
  return (
    <div className="rounded-[var(--radius-md)] border border-[var(--color-surface-border)] bg-[var(--color-surface-subtle)] p-3 flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <span className="t-label text-[var(--color-text-secondary)]">
          {comment.status === "replied" ? "IA respondeu" : "Aguardando IA…"}
        </span>
        <button
          type="button"
          onClick={onResolve}
          aria-label="Resolver"
          className="inline-flex items-center gap-1 t-body-sm text-[var(--color-text-tertiary)] hover:text-[var(--color-teal-700)]"
        >
          <CheckCircle2 className="w-3.5 h-3.5" aria-hidden />
          Resolver
        </button>
      </div>
      <p className="t-body-sm text-[var(--color-text-primary)] whitespace-pre-wrap break-words">
        {comment.text}
      </p>
      {comment.replyPreview && (
        <p className="t-body-sm italic text-[var(--color-text-secondary)] border-t border-[var(--color-surface-border)] pt-1.5">
          {comment.replyPreview}
        </p>
      )}
    </div>
  );
}

export interface BlockSpan {
  /** Trimmed markdown source for this block. */
  source: string;
  /** 1-based line number of the block's first non-blank line in the full draft. */
  startLine: number;
  /** 1-based line number of the block's last non-blank line in the full draft. */
  endLine: number;
}

/**
 * Split raw markdown into top-level blocks the way marked itself
 * does: consecutive non-blank lines form a block, blank lines
 * separate blocks. Keeps fenced code regions intact by not
 * splitting inside a fence. Records 1-based line numbers for each
 * block so the caller can pass them to the AI when a comment is
 * posted.
 */
export function splitBlocksWithLines(md: string): BlockSpan[] {
  const lines = md.split(/\r?\n/);
  const blocks: BlockSpan[] = [];
  let current: string[] = [];
  let currentStart: number | null = null;
  let currentEnd: number | null = null;
  let inFence = false;

  const flush = () => {
    if (current.length === 0) {
      currentStart = null;
      currentEnd = null;
      return;
    }
    const joined = current.join("\n").trim();
    if (joined.length > 0 && currentStart !== null && currentEnd !== null) {
      blocks.push({ source: joined, startLine: currentStart, endLine: currentEnd });
    }
    current = [];
    currentStart = null;
    currentEnd = null;
  };

  lines.forEach((line, idx) => {
    const lineNumber = idx + 1;
    if (/^\s*(?:```|~~~)/.test(line)) {
      inFence = !inFence;
      if (currentStart === null) currentStart = lineNumber;
      currentEnd = lineNumber;
      current.push(line);
      if (!inFence) flush();
      return;
    }
    if (inFence) {
      if (currentStart === null) currentStart = lineNumber;
      currentEnd = lineNumber;
      current.push(line);
      return;
    }
    if (line.trim() === "") {
      flush();
    } else {
      if (currentStart === null) currentStart = lineNumber;
      currentEnd = lineNumber;
      current.push(line);
    }
  });
  flush();
  return blocks;
}

/**
 * Back-compat shim — callers that only need the source strings.
 * Delegates to `splitBlocksWithLines` so there is one source of
 * truth for the splitting rules.
 */
export function splitBlocks(md: string): string[] {
  return splitBlocksWithLines(md).map((b) => b.source);
}
