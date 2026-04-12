"use client";

import { useState } from "react";
import { Button } from "./button";

interface CopyButtonProps {
  text: string;
  label?: string;
  className?: string;
}

export function CopyButton({ text, label = "Copy", className }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <Button variant="secondary" size="sm" onClick={handleCopy} className={className}>
      {copied ? "Copied!" : label}
    </Button>
  );
}
