"use client";

import Link from "next/link";
import type { PageResponse, TreeNode } from "@historiador/types";
import { Spinner } from "@/components/ui/spinner";
import { CollectionTreeNode } from "./collection-tree-node";

interface Props {
  tree: TreeNode[];
  pages: PageResponse[];
  selectedId: string | null;
  activePageId: string | null;
  expandedIds: Set<string>;
  isLoading: boolean;
  onSelect: (id: string | null) => void;
  onToggleExpand: (id: string) => void;
}

export function CollectionTree({
  tree,
  pages,
  selectedId,
  activePageId,
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

  const rootPages = pages.filter((p) => !p.collection_id);

  return (
    <div className="space-y-0.5">
      {/* "All Pages" root item */}
      <div
        className={`flex items-center gap-1 px-2 py-1 text-sm cursor-pointer rounded transition-colors ${
          selectedId === null
            ? "bg-primary-100 text-primary-800"
            : "hover:bg-surface-hover"
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
          pages={pages}
          selectedId={selectedId}
          activePageId={activePageId}
          expandedIds={expandedIds}
          onSelect={onSelect}
          onToggleExpand={onToggleExpand}
        />
      ))}

      {rootPages.length > 0 && (
        <div className="pt-2 mt-2 border-t border-surface-border">
          <p className="px-2 py-1 text-[10px] font-semibold text-text-tertiary uppercase tracking-wider">
            Sem coleção
          </p>
          {rootPages.map((p) => (
            <PageLink key={p.id} page={p} activePageId={activePageId} depth={0} />
          ))}
        </div>
      )}

      {tree.length === 0 && pages.length === 0 && (
        <p className="px-2 py-2 text-xs text-text-disabled">
          Nenhuma coleção ou página ainda
        </p>
      )}
    </div>
  );
}

export function PageLink({
  page,
  activePageId,
  depth,
}: {
  page: PageResponse;
  activePageId: string | null;
  depth: number;
}) {
  const isActive = page.id === activePageId;
  const title =
    (page.versions && page.versions[0]?.title) || page.slug || "Sem título";
  return (
    <Link
      href={`/dashboard/pages/${page.id}`}
      className={`flex items-center gap-1 px-2 py-1 text-sm rounded transition-colors truncate ${
        isActive
          ? "bg-primary-50 text-primary-700 font-medium"
          : "text-text-secondary hover:bg-surface-hover"
      }`}
      style={{ paddingLeft: `${depth * 16 + 24}px` }}
      title={title}
    >
      <span className="w-3 text-text-disabled">·</span>
      <span className="truncate">{title}</span>
    </Link>
  );
}
