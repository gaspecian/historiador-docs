"use client";

import { type SelectHTMLAttributes } from "react";

interface SelectOption {
  value: string;
  label: string;
}

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label?: string;
  options: SelectOption[];
  error?: string;
}

export function Select({ label, options, error, className = "", id, ...props }: SelectProps) {
  const selectId = id || label?.toLowerCase().replace(/\s+/g, "-");
  const borderClass = error
    ? "border-red-600 focus:border-red-600 focus-visible:[box-shadow:var(--shadow-focus-danger)]"
    : "border-surface-border focus:border-primary-600 focus-visible:[box-shadow:var(--shadow-focus)]";
  return (
    <div className="flex flex-col gap-1.5">
      {label && (
        <label
          htmlFor={selectId}
          className="text-xs font-semibold text-text-secondary"
        >
          {label}
        </label>
      )}
      <select
        id={selectId}
        className={`h-10 rounded-md border bg-surface-canvas px-3 text-sm text-text-primary transition-[border-color,box-shadow] focus:outline-none disabled:bg-surface-subtle disabled:text-text-disabled ${borderClass} ${className}`}
        {...props}
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
      {error && <p className="text-xs text-red-600">{error}</p>}
    </div>
  );
}
