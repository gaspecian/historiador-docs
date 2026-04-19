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
import { useCallback, useEffect, useRef } from "react";

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
  // We intentionally do not parse `initialMarkdown` through
  // prosemirror-markdown here. The server-side BlockTree serializer
  // is authoritative; the first cut of A5 shows the raw markdown in
  // a prose-mirror-friendly form by using Tiptap's own parser for
  // headings / paragraphs. Richer round-trip ships alongside A10
  // when proposals need precise anchoring.
  const initialHtml = markdownToPlainHtml(initialMarkdown);

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
          "prose max-w-none focus:outline-none min-h-[60vh] p-6 " +
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
 * Minimal markdown → HTML for the initial Tiptap content. Handles
 * headings and paragraphs; anything more exotic falls through as a
 * paragraph. This is *display-only* — the server's BlockTree is the
 * source of truth for structure, so lossy rendering here is OK
 * until A10 upgrades the round-trip.
 */
function markdownToPlainHtml(md: string): string {
  const blocks = md.split(/\n\s*\n/);
  return blocks
    .map((block) => {
      const trimmed = block.trim();
      if (trimmed.length === 0) return "";
      // Strip block-id HTML comments — they are metadata, not content.
      const withoutIdComment = trimmed.replace(/^\s*<!--\s*block:[0-9a-fA-F-]+\s*-->\s*/m, "");
      if (withoutIdComment.length === 0) return "";
      const headingMatch = withoutIdComment.match(/^(#{1,6})\s+(.*)$/);
      if (headingMatch) {
        const level = headingMatch[1].length;
        return `<h${level}>${escapeHtml(headingMatch[2])}</h${level}>`;
      }
      return `<p>${escapeHtml(withoutIdComment)}</p>`;
    })
    .filter(Boolean)
    .join("");
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
