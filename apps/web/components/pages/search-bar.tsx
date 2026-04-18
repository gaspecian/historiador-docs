"use client";

import { useEffect, useRef, useState } from "react";

interface Props {
 onSearch: (query: string) => void;
}

export function SearchBar({ onSearch }: Props) {
 const [query, setQuery] = useState("");
 const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

 useEffect(() => {
 clearTimeout(debounceRef.current);
 debounceRef.current = setTimeout(() => {
 onSearch(query);
 }, 300);
 return () => clearTimeout(debounceRef.current);
 }, [query, onSearch]);

 return (
 <input
 type="search"
 placeholder="Search pages..."
 value={query}
 onChange={(e) => setQuery(e.target.value)}
 className="w-full max-w-xs rounded border border-surface-border-strong px-3 py-1.5 text-sm bg-white focus:outline-none focus:ring-2 focus:ring-primary-500"
 />
 );
}
