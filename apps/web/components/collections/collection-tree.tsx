"use client";

import type { TreeNode } from "@/lib/types";
import { Spinner } from "@/components/ui/spinner";
import { CollectionTreeNode } from "./collection-tree-node";

interface Props {
  tree: TreeNode[];
  selectedId: string | null;
  expandedIds: Set<string>;
  isLoading: boolean;
  onSelect: (id: string | null) => void;
  onToggleExpand: (id: string) => void;
}

export function CollectionTree({
  tree,
  selectedId,
  expandedIds,
  isLoading,
  onSelect,
  onToggleExpand,
}: Props) {
  if (isLoading) {
    return (
      <div className="flex justify-center py-4">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="space-y-0.5">
      {/* "All Pages" root item */}
      <div
        className={`flex items-center gap-1 px-2 py-1 text-sm cursor-pointer rounded transition-colors ${
          selectedId === null
            ? "bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200"
            : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
        }`}
        onClick={() => onSelect(null)}
      >
        <span className="w-4" />
        <span className="font-medium">All Pages</span>
      </div>

      {tree.map((node) => (
        <CollectionTreeNode
          key={node.id}
          node={node}
          depth={0}
          selectedId={selectedId}
          expandedIds={expandedIds}
          onSelect={onSelect}
          onToggleExpand={onToggleExpand}
        />
      ))}

      {tree.length === 0 && (
        <p className="px-2 py-2 text-xs text-zinc-400">No collections yet</p>
      )}
    </div>
  );
}
