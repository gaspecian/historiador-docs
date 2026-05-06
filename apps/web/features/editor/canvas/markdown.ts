/**
 * Canvas markdown serialize / parse (Sprint 11, phase A5).
 *
 * Mirrors `crates/blocks` on the client side so the round-trip
 * through Tiptap preserves block-ID comments:
 *
 * ```markdown
 * <!-- block:01966c... -->
 *
 * # Heading
 *
 * <!-- block:01966d... -->
 *
 * A paragraph.
 * ```
 *
 * The server is the source of truth — when a page loads, the server
 * already returns markdown with `<!-- block:... -->` prefixes (after
 * the A2 backfill). We parse them here into a shape Tiptap can
 * consume, and on save we emit the same format so the server's
 * `parse_markdown` round-trips cleanly.
 *
 * This is intentionally narrow. prosemirror-markdown handles full
 * markdown; we layer block-ID preservation on top.
 */

import { defaultMarkdownParser, defaultMarkdownSerializer } from "prosemirror-markdown";
import type { Node as ProseMirrorNode } from "@tiptap/pm/model";

const BLOCK_ID_COMMENT = /^\s*<!--\s*block:([0-9a-fA-F-]+)\s*-->\s*$/;

/**
 * Split a raw markdown string into chunks keyed by block ID.
 * Returns an array of { id, markdown } where `markdown` contains the
 * block body (without the id comment or surrounding blanks).
 */
export function splitByBlockId(raw: string): Array<{ id: string | null; markdown: string }> {
  const lines = raw.split(/\r?\n/);
  const out: Array<{ id: string | null; markdown: string }> = [];
  let currentId: string | null = null;
  let buffer: string[] = [];

  const flush = () => {
    if (buffer.length === 0) return;
    const md = buffer.join("\n").trim();
    if (md.length > 0) {
      out.push({ id: currentId, markdown: md });
    }
    buffer = [];
    currentId = null;
  };

  for (const line of lines) {
    const match = line.match(BLOCK_ID_COMMENT);
    if (match) {
      flush();
      currentId = match[1];
      continue;
    }
    if (line.trim() === "" && buffer.length === 0) continue;
    buffer.push(line);
  }
  flush();
  return out;
}

/**
 * Walk a ProseMirror doc and emit markdown with `<!-- block:... -->`
 * comments before every top-level block node. Uses the default
 * markdown serializer for each block's body.
 */
export function serializeCanvas(doc: ProseMirrorNode): string {
  const parts: string[] = [];
  doc.forEach((child) => {
    const blockId = (child.attrs as { blockId?: string | null }).blockId ?? null;
    const body = serializeNode(child).trim();
    if (body.length === 0) return;
    if (blockId) {
      parts.push(`<!-- block:${blockId} -->`);
      parts.push("");
    }
    parts.push(body);
    parts.push("");
  });
  return parts.join("\n").trimEnd() + "\n";
}

function serializeNode(node: ProseMirrorNode): string {
  // prosemirror-markdown serializes a document, so wrap the block in
  // a pseudo-doc and strip trailing whitespace.
  const pseudoDoc = node.type.schema.topNodeType.create({}, node);
  return defaultMarkdownSerializer.serialize(pseudoDoc);
}

/**
 * Parse markdown into a ProseMirror doc. Preserves block IDs from
 * the comments: after parsing, each top-level node is matched against
 * the split-by-id sequence and the matching ID is applied as an
 * attribute by the caller (see `canvas.tsx`).
 */
export { defaultMarkdownParser };
