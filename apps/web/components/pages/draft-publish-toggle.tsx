"use client";

import { useState } from "react";
import { apiFetch } from "@/lib/api";
import { Button } from "@/components/ui/button";
import type { PageStatus } from "@/lib/types";

interface Props {
  pageId: string;
  status: PageStatus;
  onToggled: () => void;
}

export function DraftPublishToggle({ pageId, status, onToggled }: Props) {
  const [loading, setLoading] = useState(false);

  const handleToggle = async () => {
    setLoading(true);
    try {
      const endpoint = status === "draft" ? "publish" : "draft";
      await apiFetch(`/pages/${pageId}/${endpoint}`, { method: "POST" });
      onToggled();
    } catch {
      // Error handled silently for alpha
    } finally {
      setLoading(false);
    }
  };

  return (
    <Button
      variant={status === "draft" ? "primary" : "secondary"}
      size="sm"
      onClick={handleToggle}
      disabled={loading}
    >
      {loading ? "..." : status === "draft" ? "Publish" : "Unpublish"}
    </Button>
  );
}
