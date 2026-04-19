"use client";

import type { PageResponse, TreeNode } from "@historiador/types";
import { PageLink } from "./collection-tree";

interface Props {
  node: TreeNode;
  depth: number;
  pages: PageResponse[];
  selectedId: string | null;
  activePageId: string | null;
  expandedIds: Set<string>;
  onSelect: (id: string | null) => void;
  onToggleExpand: (id: string) => void;
}

export function CollectionTreeNode({
  node,
  depth,
  pages,
  selectedId,
  activePageId,
  expandedIds,
  onSelect,
  onToggleExpand,
}: Props) {
  const childCollections = node.children;
  const collectionPages = pages.filter((p) => p.collection_id === node.id);
  const hasExpandable = childCollections.length > 0 || collectionPages.length > 0;
  const isExpanded = expandedIds.has(node.id);
  const isSelected = selectedId === node.id;

  return (
    <div>
      <div
        className={`flex items-center gap-1 px-2 py-1 text-sm cursor-pointer rounded transition-colors ${
          isSelected
            ? "bg-primary-100 text-primary-800"
            : "hover:bg-surface-hover"
        }`}
        style={{ paddingLeft: `${depth * 16 + 8}px` }}
        onClick={() => onSelect(node.id)}
      >
        {hasExpandable ? (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onToggleExpand(node.id);
            }}
            className="w-4 h-4 flex items-center justify-center text-text-disabled hover:text-text-secondary"
          >
            {isExpanded ? "\u25BE" : "\u25B8"}
          </button>
        ) : (
          <span className="w-4" />
        )}
        <span className="truncate">{node.name}</span>
        {collectionPages.length > 0 && (
          <span className="ml-auto text-[10px] text-text-tertiary">
            {collectionPages.length}
          </span>
        )}
      </div>
      {hasExpandable && isExpanded && (
        <div>
          {childCollections.map((child) => (
            <CollectionTreeNode
              key={child.id}
              node={child}
              depth={depth + 1}
              pages={pages}
              selectedId={selectedId}
              activePageId={activePageId}
              expandedIds={expandedIds}
              onSelect={onSelect}
              onToggleExpand={onToggleExpand}
            />
          ))}
          {collectionPages.map((p) => (
            <PageLink
              key={p.id}
              page={p}
              activePageId={activePageId}
              depth={depth + 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}
