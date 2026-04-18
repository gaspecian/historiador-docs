"use client";

import { type InputHTMLAttributes } from "react";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
}

export function Input({ label, error, className = "", id, ...props }: InputProps) {
  const inputId = id || label?.toLowerCase().replace(/\s+/g, "-");
  const borderClass = error
    ? "border-red-600 focus:border-red-600 focus-visible:[box-shadow:var(--shadow-focus-danger)]"
    : "border-surface-border focus:border-primary-600 focus-visible:[box-shadow:var(--shadow-focus)]";
  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <label
          htmlFor={inputId}
          className="text-xs font-semibold text-text-secondary"
        >
          {label}
        </label>
      )}
      <input
        id={inputId}
        className={`h-10 rounded-md border bg-surface-canvas px-3 text-sm text-text-primary placeholder:text-text-disabled transition-[border-color,box-shadow] focus:outline-none disabled:bg-surface-subtle disabled:text-text-disabled ${borderClass} ${className}`}
        {...props}
      />
      {error && <p className="text-xs text-red-600">{error}</p>}
    </div>
  );
}
