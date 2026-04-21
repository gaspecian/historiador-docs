"use client";

import { createContext, useContext, useMemo, useState } from "react";

interface CollectionSelectionValue {
  selectedId: string | null;
  setSelectedId: (id: string | null) => void;
}

const CollectionSelectionContext = createContext<CollectionSelectionValue | null>(null);

export function CollectionSelectionProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const value = useMemo(() => ({ selectedId, setSelectedId }), [selectedId]);
  return (
    <CollectionSelectionContext.Provider value={value}>
      {children}
    </CollectionSelectionContext.Provider>
  );
}

export function useCollectionSelection(): CollectionSelectionValue {
  const ctx = useContext(CollectionSelectionContext);
  if (!ctx) {
    throw new Error(
      "useCollectionSelection must be used inside <CollectionSelectionProvider>",
    );
  }
  return ctx;
}
