"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

function BookMark({ size = 24 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M16 7 V25" />
      <path d="M16 7 C 12 5, 8 5, 4.5 6 V 24 C 8 23, 12 23, 16 25" />
      <path d="M16 7 C 20 5, 24 5, 27.5 6 V 24 C 24 23, 20 23, 16 25" />
      <path d="M22 10 L 26 14" />
    </svg>
  );
}

function NetworkArt() {
  return (
    <svg
      viewBox="0 0 200 200"
      fill="none"
      stroke="currentColor"
      strokeWidth={1}
      strokeLinecap="round"
      className="w-80 h-80 opacity-50 text-primary-600"
    >
      <circle cx="100" cy="100" r="6" />
      <circle cx="40" cy="50" r="4" />
      <circle cx="160" cy="60" r="4" />
      <circle cx="50" cy="150" r="4" />
      <circle cx="150" cy="150" r="4" />
      <circle cx="100" cy="30" r="3" />
      <circle cx="30" cy="110" r="3" />
      <circle cx="170" cy="110" r="3" />
      <path d="M100 100 L40 50 M100 100 L160 60 M100 100 L50 150 M100 100 L150 150 M100 100 L100 30 M100 100 L30 110 M100 100 L170 110" />
    </svg>
  );
}

export default function LoginPage() {
  const router = useRouter();
  const { login } = useAuth();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      await login(email, password);
      router.push("/dashboard/pages");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <main className="grid h-screen grid-cols-1 md:grid-cols-2">
      <div
        className="relative hidden flex-col p-10 md:flex"
        style={{ background: "var(--color-primary-50)" }}
      >
        <div
          className="flex items-center gap-2.5 text-text-primary"
          style={{ fontFamily: "var(--font-display)", fontStyle: "italic", fontSize: 22 }}
        >
          <span className="text-primary-600">
            <BookMark />
          </span>
          Nexian Tech
        </div>
        <div className="flex-1 flex items-center justify-center">
          <NetworkArt />
        </div>
      </div>

      <div className="flex items-center justify-center bg-surface-canvas p-10">
        <form onSubmit={handleSubmit} className="w-[360px]">
          <div
            className="mb-7 text-[13px] text-text-tertiary"
            style={{ fontFamily: "var(--font-mono)", letterSpacing: "0.04em" }}
          >
            historiador.doc
          </div>

          <h1
            className="mb-2"
            style={{
              fontFamily: "var(--font-display)",
              fontSize: 44,
              fontWeight: 400,
              lineHeight: 1.1,
              color: "var(--color-text-primary)",
              margin: 0,
            }}
          >
            Bem-vindo <em>de volta</em>.
          </h1>
          <div className="mb-7 text-[15px] text-text-secondary">
            Entre para continuar documentando.
          </div>

          <div className="space-y-3.5">
            <Input
              label="E-mail"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              required
              autoComplete="email"
            />
            <Input
              label="Senha"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              autoComplete="current-password"
              error={error || undefined}
            />
          </div>

          <Button
            type="submit"
            disabled={loading}
            className="mt-4 w-full h-11 justify-center"
          >
            {loading ? "Entrando…" : "Entrar"}
          </Button>

          <div className="mt-3.5 text-center">
            <a className="text-[13px] text-primary-600 no-underline hover:underline" href="#">
              Esqueci minha senha
            </a>
          </div>
        </form>
      </div>
    </main>
  );
}
