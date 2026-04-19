/**
 * Tiptap / ProseMirror extension that adds a stable `data-block-id`
 * attribute to every top-level node (headings, paragraphs, lists,
 * code blocks, tables, blockquotes).
 *
 * IDs are UUIDv7 (minted client-side via the `uuid` package), so the
 * ordering in the document matches creation time — useful for debug
 * logs even though document order is authoritative.
 *
 * The extension hooks into every editor transaction: any top-level
 * node without an ID gets one on the next transaction. On a fresh
 * paste the user sees IDs materialise instantly. Editing an existing
 * block preserves its ID.
 */

import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { v7 as uuidv7 } from "uuid";

/** Node types that get their own block ID. */
const BLOCK_NODES = new Set([
  "heading",
  "paragraph",
  "bulletList",
  "orderedList",
  "codeBlock",
  "blockquote",
  "table",
]);

export const BlockIdExtension = Extension.create({
  name: "blockId",

  addGlobalAttributes() {
    return [
      {
        types: Array.from(BLOCK_NODES),
        attributes: {
          blockId: {
            default: null,
            parseHTML: (element) => element.getAttribute("data-block-id"),
            renderHTML: (attributes) => {
              const id = attributes.blockId as string | null;
              if (!id) return {};
              return { "data-block-id": id };
            },
          },
        },
      },
    ];
  },

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: new PluginKey("blockIdMinter"),
        appendTransaction: (_transactions, _oldState, newState) => {
          let changed = false;
          const tr = newState.tr;
          newState.doc.descendants((node, pos) => {
            if (!BLOCK_NODES.has(node.type.name)) return;
            if (pos > 0 && newState.doc.resolve(pos).depth !== 1) return;
            const existing = node.attrs.blockId as string | null | undefined;
            if (!existing) {
              tr.setNodeMarkup(pos, undefined, {
                ...node.attrs,
                blockId: uuidv7(),
              });
              changed = true;
            }
          });
          return changed ? tr : null;
        },
      }),
    ];
  },
});
