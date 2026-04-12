"use client";

import { Badge } from "@/components/ui/badge";
import type { PageVersion } from "@/lib/types";

interface Props {
  versions: PageVersion[];
  workspaceLanguages: string[];
}

export function LanguageBadges({ versions, workspaceLanguages }: Props) {
  const existingLanguages = new Set(versions.map((v) => v.language));

  return (
    <div className="flex gap-1 flex-wrap">
      {workspaceLanguages.map((lang) => (
        <Badge
          key={lang}
          variant={existingLanguages.has(lang) ? "success" : "neutral"}
        >
          {lang}
        </Badge>
      ))}
    </div>
  );
}
