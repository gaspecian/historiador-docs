"use client";

import { useState } from "react";
import { apiFetch } from "@/lib/api";
import { PublishConfirmModal } from "./publish-confirm-modal";
import { Button } from "@/components/ui/button";
import type { PageStatus } from "@historiador/types";

interface Props {
  pageId: string;
  status: PageStatus;
  workspaceLanguages?: string[];
  versionLanguages?: string[];
  onToggled: () => void;
}

export function DraftPublishToggle({
  pageId,
  status,
  workspaceLanguages = [],
  versionLanguages = [],
  onToggled,
}: Props) {
  const [loading, setLoading] = useState(false);
  const [showModal, setShowModal] = useState(false);

  const missingLanguages = workspaceLanguages.filter(
    (lang) => !versionLanguages.includes(lang),
  );

  const handleClick = () => {
    if (status === "published") {
      doToggle();
      return;
    }
    if (missingLanguages.length > 0) {
      setShowModal(true);
    } else {
      doToggle();
    }
  };

  const doToggle = async () => {
    setShowModal(false);
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
    <>
      <Button
        variant={status === "draft" ? "primary" : "secondary"}
        size="sm"
        onClick={handleClick}
        disabled={loading}
      >
        {loading ? "..." : status === "draft" ? "Publish" : "Unpublish"}
      </Button>
      <PublishConfirmModal
        open={showModal}
        missingLanguages={missingLanguages}
        onConfirm={doToggle}
        onCancel={() => setShowModal(false)}
      />
    </>
  );
}
