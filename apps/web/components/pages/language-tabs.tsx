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
 <div className="flex gap-1 border-b border-surface-border">
 {workspaceLanguages.map((lang) => {
 const exists = existingLanguages.has(lang);
 const isActive = lang === activeLanguage;

 return (
 <button
 key={lang}
 onClick={() => onSelect(lang)}
 className={`flex items-center gap-1.5 px-3 py-1.5 text-sm border-b-2 transition-colors ${
 isActive
 ? "border-primary-600 text-primary-600"
 : "border-transparent text-text-tertiary hover:text-text-secondary"
 }`}
 >
 {lang}
 {exists ? (
 <span className="inline-block w-2 h-2 rounded-full bg-teal-500" title="Version exists" />
 ) : (
 <span className="inline-block w-2 h-2 rounded-full bg-amber-500" title="Missing version" />
 )}
 </button>
 );
 })}
 </div>
 );
}
