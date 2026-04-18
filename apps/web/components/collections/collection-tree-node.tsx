"use client";

import type { TreeNode } from "@historiador/types";

interface Props {
 node: TreeNode;
 depth: number;
 selectedId: string | null;
 expandedIds: Set<string>;
 onSelect: (id: string | null) => void;
 onToggleExpand: (id: string) => void;
}

export function CollectionTreeNode({
 node,
 depth,
 selectedId,
 expandedIds,
 onSelect,
 onToggleExpand,
}: Props) {
 const hasChildren = node.children.length > 0;
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
 {/* Expand/collapse toggle */}
 {hasChildren ? (
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
 </div>
 {hasChildren && isExpanded && (
 <div>
 {node.children.map((child) => (
 <CollectionTreeNode
 key={child.id}
 node={child}
 depth={depth + 1}
 selectedId={selectedId}
 expandedIds={expandedIds}
 onSelect={onSelect}
 onToggleExpand={onToggleExpand}
 />
 ))}
 </div>
 )}
 </div>
 );
}
