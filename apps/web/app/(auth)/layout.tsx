"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { status } from "@/lib/services/setup";

export default function AuthLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const [checked, setChecked] = useState(false);

  useEffect(() => {
    let cancelled = false;
    status()
      .then((s) => {
        if (cancelled) return;
        if (!s.setup_complete) router.replace("/setup");
        else setChecked(true);
      })
      .catch(() => {
        if (!cancelled) setChecked(true);
      });
    return () => {
      cancelled = true;
    };
  }, [router]);

  if (!checked) return null;
  return <>{children}</>;
}
