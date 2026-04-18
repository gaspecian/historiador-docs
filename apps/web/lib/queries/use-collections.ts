"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import * as collectionsService from "@/lib/services/collections";
import type {
  CreateCollectionBody,
  UpdateCollectionBody,
} from "@/lib/services/collections";
import type { CollectionResponse, TreeNode } from "@historiador/types";
import { queryKeys } from "./keys";

function buildTree(collections: CollectionResponse[]): TreeNode[] {
  const childrenMap = new Map<string | null, CollectionResponse[]>();
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

export function useCollectionsQuery() {
  const query = useQuery({
    queryKey: queryKeys.collections.list(),
    queryFn: () => collectionsService.list(),
  });
  const tree = useMemo(() => buildTree(query.data ?? []), [query.data]);
  return { ...query, tree };
}

export function useCreateCollectionMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: CreateCollectionBody) => collectionsService.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.collections.all });
    },
  });
}

export function useUpdateCollectionMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateCollectionBody }) =>
      collectionsService.update(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.collections.all });
    },
  });
}

export function useDeleteCollectionMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => collectionsService.remove(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.collections.all });
      // Deleting a collection cascades to its pages.
      qc.invalidateQueries({ queryKey: queryKeys.pages.all });
    },
  });
}

