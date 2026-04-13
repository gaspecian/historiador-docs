"use client";

import { Badge } from "@/components/ui/badge";
import type { PageVersion } from "@historiador/types";

interface Props {
  versions: PageVersion[];
  workspaceLanguages: string[];
  pageId?: string;
  onMissingClick?: (pageId: string, lang: string) => void;
}

export function LanguageBadges({ versions, workspaceLanguages, pageId, onMissingClick }: Props) {
  const existingLanguages = new Set(versions.map((v) => v.language));

  return (
    <div className="flex gap-1 flex-wrap">
      {workspaceLanguages.map((lang) => {
        const exists = existingLanguages.has(lang);

        if (exists) {
          return (
            <Badge key={lang} variant="success">
              {lang}
            </Badge>
          );
        }

        return (
          <Badge
            key={lang}
            variant="warning"
            title={`Missing — click to create ${lang} version`}
            className={pageId && onMissingClick ? "cursor-pointer" : undefined}
            onClick={
              pageId && onMissingClick
                ? () => onMissingClick(pageId, lang)
                : undefined
            }
          >
            {lang}
          </Badge>
        );
      })}
    </div>
  );
}
