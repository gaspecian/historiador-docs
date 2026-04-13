"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { apiFetch } from "./api";
import type { Collection, TreeNode } from "@historiador/types";

function buildTree(collections: Collection[]): TreeNode[] {
  const childrenMap = new Map<string | null, Collection[]>();
  for (const c of collections) {
    const key = c.parent_id ?? "root";
    if (!childrenMap.has(key)) childrenMap.set(key, []);
    childrenMap.get(key)!.push(c);
  }

  function recurse(parentId: string | null): TreeNode[] {
    const key = parentId ?? "root";
    return (childrenMap.get(key) ?? []).map((c) => ({
      ...c,
      children: recurse(c.id),
    }));
  }

  return recurse(null);
}

export function useCollections() {
  const [collections, setCollections] = useState<Collection[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchCollections = useCallback(async () => {
    try {
      setIsLoading(true);
      const data = await apiFetch<Collection[]>("/collections");
      setCollections(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load collections");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchCollections();
  }, [fetchCollections]);

  const tree = useMemo(() => buildTree(collections), [collections]);

  const toggleExpanded = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  return {
    collections,
    tree,
    selectedId,
    setSelectedId,
    expandedIds,
    toggleExpanded,
    isLoading,
    error,
    refresh: fetchCollections,
  };
}
