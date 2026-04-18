import { type HTMLAttributes } from "react";

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  elevation?: "flat" | "raised";
  padded?: boolean;
}

export function Card({
  elevation = "flat",
  padded = true,
  className = "",
  ...props
}: CardProps) {
  const shadow = elevation === "raised" ? "shadow-md" : "";
  const padding = padded ? "p-5" : "";
  return (
    <div
      className={`rounded-lg border border-surface-border bg-surface-canvas ${shadow} ${padding} ${className}`}
      {...props}
    />
  );
}
