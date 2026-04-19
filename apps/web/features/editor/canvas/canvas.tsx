"use client";

/**
 * The Sprint 11 canvas (ADR-008, ADR-010).
 *
 * A Tiptap / ProseMirror editor with:
 *   - StarterKit nodes (heading, paragraph, list, code, blockquote)
 *   - Our BlockIdExtension, which mints UUIDv7 `data-block-id`s onto
 *     every top-level node and preserves them across edits
 *   - A markdown bridge that emits `<!-- block:<uuid> -->` comments on
 *     save so `crates/blocks::parse_markdown` rebinds IDs verbatim
 *   - 30-second debounced auto-save of the base document only
 *     (proposal overlay lives in a separate layer, added in A10)
 */

import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { marked } from "marked";
import { useCallback, useEffect, useMemo, useRef } from "react";

import { BlockIdExtension } from "./block-id-extension";
import { serializeCanvas } from "./markdown";

const AUTO_SAVE_MS = 30_000;

export interface CanvasProps {
  /** Initial markdown content (already has `<!-- block:... -->` ids). */
  initialMarkdown: string;
  /** Called on 30-second debounced save. Receives serialized markdown. */
  onSave: (markdown: string) => void | Promise<void>;
  /** Called synchronously on every change — use for live preview etc. */
  onChange?: (markdown: string) => void;
}

export function Canvas({ initialMarkdown, onSave, onChange }: CanvasProps) {
  // Parse markdown → HTML once per new document. `marked` handles
  // the full GFM subset the server's BlockTree supports; we strip
  // `<!-- block:<uuid> -->` comments first so Tiptap does not see
  // them as content (our BlockIdExtension mints fresh IDs on the
  // next transaction, and the server's parse_markdown will bind
  // the IDs back on save per the A2 round-trip).
  const initialHtml = useMemo(() => markdownToHtml(initialMarkdown), [initialMarkdown]);

  const saveTimerRef = useRef<number | null>(null);
  const latestMarkdownRef = useRef<string>(initialMarkdown);

  const flush = useCallback(() => {
    if (saveTimerRef.current != null) {
      window.clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
    void onSave(latestMarkdownRef.current);
  }, [onSave]);

  const editor = useEditor({
    extensions: [StarterKit, BlockIdExtension],
    content: initialHtml,
    immediatelyRender: false,
    editorProps: {
      attributes: {
        class:
          "md-prose focus:outline-none min-h-[60vh] p-6 " +
          "bg-[var(--color-surface-canvas)] text-[var(--color-text-primary)] " +
          "rounded-[var(--radius-lg)] border border-[var(--color-surface-border)]",
      },
    },
    onUpdate: ({ editor }) => {
      const md = serializeCanvas(editor.state.doc);
      latestMarkdownRef.current = md;
      onChange?.(md);
      if (saveTimerRef.current != null) {
        window.clearTimeout(saveTimerRef.current);
      }
      saveTimerRef.current = window.setTimeout(() => {
        void onSave(md);
      }, AUTO_SAVE_MS);
    },
  });

  useEffect(() => {
    return () => {
      // Flush pending save on unmount so partial edits do not vanish.
      flush();
    };
  }, [flush]);

  if (!editor) {
    return (
      <div className="p-6 text-[var(--color-text-tertiary)]">Loading canvas…</div>
    );
  }
  return <EditorContent editor={editor} />;
}

/**
 * Markdown → HTML for Tiptap's initial content. Strips the
 * `<!-- block:<uuid> -->` comments (metadata, not content), then
 * normalises the source so LLM output without blank-line
 * separators still parses as structured markdown, then hands it to
 * `marked`. The BlockIdExtension mints fresh IDs on the next
 * transaction; the server rebinds them on save via `crates/blocks`.
 *
 * `marked.parse` is called synchronously with `async: false` so the
 * return type is a `string` rather than a `Promise`.
 */
function markdownToHtml(md: string): string {
  const withoutIdComments = md.replace(/<!--\s*block:[0-9a-fA-F-]+\s*-->/g, "");
  const normalised = normaliseMarkdown(withoutIdComments);
  const html = marked.parse(normalised, { async: false, gfm: true, breaks: false });
  return typeof html === "string" ? html : "";
}

/**
 * Make marked's life easier with LLM output that skips blank
 * lines. Ensures a blank line sits between block-level constructs
 * so setext headings, code fences, lists, and paragraphs don't run
 * together. Does not mutate the structure of the content — only
 * whitespace.
 */
function normaliseMarkdown(md: string): string {
  const lines = md.split(/\r?\n/);
  const out: string[] = [];
  const isBlank = (s: string) => s.trim().length === 0;
  const isAtxHeading = (s: string) => /^#{1,6}\s+\S/.test(s);
  const isSetextUnderline = (s: string) => /^(=+|-+)\s*$/.test(s) && s.trim().length >= 2;
  const isFence = (s: string) => /^```/.test(s.trimStart()) || /^~~~/.test(s.trimStart());
  const isListItem = (s: string) => /^\s*(?:[-*+]\s+|\d+\.\s+)/.test(s);

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const prev = out.length > 0 ? out[out.length - 1] : "";
    const next = lines[i + 1] ?? "";

    // Blank line before an ATX heading.
    if (isAtxHeading(line) && out.length > 0 && !isBlank(prev)) {
      out.push("");
    }

    // Blank line before a fenced code block opener.
    if (isFence(line) && out.length > 0 && !isBlank(prev)) {
      out.push("");
    }

    // Setext underline: ensure blank line AFTER so content that
    // follows doesn't swallow the heading.
    if (isSetextUnderline(line)) {
      out.push(line);
      if (!isBlank(next)) {
        out.push("");
      }
      continue;
    }

    out.push(line);

    // Blank line after an ATX heading.
    if (isAtxHeading(line) && !isBlank(next)) {
      out.push("");
    }

    // Blank line before a list when the previous non-list line was
    // prose (skips the case where the list itself is continuing).
    if (isListItem(next) && !isBlank(line) && !isListItem(line)) {
      out.push("");
    }
  }

  return out.join("\n");
}
