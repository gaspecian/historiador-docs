"use client";

import { useState } from "react";
import * as collectionsService from "@/lib/services/collections";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  parentId?: string | null;
  onCreated: () => void;
  onCancel: () => void;
}

export function CreateCollectionDialog({ parentId, onCreated, onCancel }: Props) {
  const [name, setName] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    setLoading(true);
    setError("");
    try {
      await collectionsService.create({
        name: name.trim(),
        parent_id: parentId || null,
      });
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create collection");
    } finally {
      setLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="p-2 space-y-2">
      <Input
        placeholder="Collection name"
        value={name}
        onChange={(e) => setName(e.target.value)}
        autoFocus
      />
      {error && <p className="text-xs text-red-600">{error}</p>}
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={loading || !name.trim()}>
          {loading ? "Creating..." : "Create"}
        </Button>
        <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
