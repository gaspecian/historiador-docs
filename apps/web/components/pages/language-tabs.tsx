"use client";

import type { PageVersionResponse } from "@historiador/types";

interface Props {
  workspaceLanguages: string[];
  versions: PageVersionResponse[];
  activeLanguage: string | null;
  onSelect: (lang: string) => void;
}

export function LanguageTabs({ workspaceLanguages, versions, activeLanguage, onSelect }: Props) {
  const existingLanguages = new Set(versions.map((v) => v.language));

  if (workspaceLanguages.length <= 1 && versions.length <= 1) {
    return null;
  }

  return (
    <div className="flex gap-1 border-b border-zinc-200 dark:border-zinc-700">
      {workspaceLanguages.map((lang) => {
        const exists = existingLanguages.has(lang);
        const isActive = lang === activeLanguage;

        return (
          <button
            key={lang}
            onClick={() => onSelect(lang)}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-sm border-b-2 transition-colors ${
              isActive
                ? "border-blue-600 text-blue-600"
                : "border-transparent text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300"
            }`}
          >
            {lang}
            {exists ? (
              <span className="inline-block w-2 h-2 rounded-full bg-green-500" title="Version exists" />
            ) : (
              <span className="inline-block w-2 h-2 rounded-full bg-amber-500" title="Missing version" />
            )}
          </button>
        );
      })}
    </div>
  );
}
