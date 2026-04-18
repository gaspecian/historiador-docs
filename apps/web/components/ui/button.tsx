"use client";

import { type ButtonHTMLAttributes } from "react";

const variants = {
  primary:
    "bg-primary-600 text-white hover:bg-primary-700 shadow-xs disabled:opacity-40",
  secondary:
    "bg-surface-canvas text-text-primary border border-surface-border hover:bg-surface-hover shadow-xs disabled:opacity-40",
  ghost:
    "bg-transparent text-text-secondary hover:bg-surface-subtle disabled:opacity-40",
  danger:
    "bg-red-600 text-white hover:bg-red-700 disabled:opacity-40",
  link:
    "bg-transparent text-primary-600 hover:underline p-0 h-auto border-0 disabled:opacity-40",
} as const;

const sizes = {
  sm: "h-8 px-3 text-[13px]",
  md: "h-10 px-4 text-sm",
  lg: "h-12 px-6 text-[15px]",
} as const;

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof variants;
  size?: keyof typeof sizes;
}

export function Button({
  variant = "primary",
  size = "md",
  className = "",
  ...props
}: ButtonProps) {
  const sizeClass = variant === "link" ? "" : sizes[size];
  return (
    <button
      className={`inline-flex items-center gap-2 rounded-md font-medium whitespace-nowrap transition-colors disabled:cursor-not-allowed focus-visible:outline-none focus-visible:[box-shadow:var(--shadow-focus)] ${variants[variant]} ${sizeClass} ${className}`}
      {...props}
    />
  );
}
