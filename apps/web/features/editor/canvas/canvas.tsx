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
 * Markdown → HTML for Tiptap's initial content. Strips the
 * `<!-- block:<uuid> -->` comments (metadata, not content) before
 * handing the source to `marked` so the parser never emits them as
 * text. The BlockIdExtension re-mints IDs on the next transaction
 * and the server rebinds them on save via `crates/blocks`.
 *
 * `marked.parse` is called synchronously with `async: false` so the
 * return type is a `string` rather than a `Promise`.
 */
function markdownToHtml(md: string): string {
  const withoutIdComments = md.replace(/<!--\s*block:[0-9a-fA-F-]+\s*-->/g, "");
  const html = marked.parse(withoutIdComments, { async: false, gfm: true });
  return typeof html === "string" ? html : "";
}
