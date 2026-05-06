"use client";

const variants = {
  success: "bg-teal-50 text-teal-700",
  warning: "bg-amber-50 text-amber-700",
  danger:  "bg-red-50 text-red-600",
  info:    "bg-primary-50 text-primary-700",
  neutral: "bg-surface-subtle text-text-secondary",
} as const;

interface BadgeProps {
  variant?: keyof typeof variants;
  children: React.ReactNode;
  className?: string;
  title?: string;
  onClick?: () => void;
}

export function Badge({
  variant = "neutral",
  children,
  className = "",
  title,
  onClick,
}: BadgeProps) {
  return (
    <span
      className={`inline-flex h-[22px] items-center gap-1.5 rounded-full px-2.5 text-xs font-semibold ${variants[variant]} ${className}`}
      title={title}
      onClick={onClick}
      role={onClick ? "button" : undefined}
    >
      {children}
    </span>
  );
}
