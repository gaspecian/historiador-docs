import type { Metadata } from "next";
import styles from "./design-system.module.css";

export const metadata: Metadata = {
  title: "Historiador Doc — Design System",
};

type Card = { ttl: string; sub: string };
type Section = {
  num: string;
  title: string;
  titleEm: string;
  count?: string;
  grid: "grid2" | "grid3";
  cards: Card[];
};

const sections: Section[] = [
  {
    num: "01",
    title: "",
    titleEm: "Marca",
    count: "2 itens",
    grid: "grid2",
    cards: [
      { ttl: "Logo & lockup", sub: "O símbolo do historiador: livro aberto com pena." },
      { ttl: "Iconografia", sub: "Lucide, 2px, cantos arredondados." },
    ],
  },
  {
    num: "02",
    title: "",
    titleEm: "Tipografia",
    count: "3 itens",
    grid: "grid3",
    cards: [
      { ttl: "Display", sub: "Instrument Serif — só momentos editoriais." },
      { ttl: "Títulos", sub: "Inter 600, escala H1–H5." },
      { ttl: "Corpo & Mono", sub: "Inter 400/500 · JetBrains Mono para código." },
    ],
  },
  {
    num: "03",
    title: "",
    titleEm: "Cor",
    count: "4 itens",
    grid: "grid2",
    cards: [
      { ttl: "Primária — Indigo", sub: "50–800. Indigo 600 é a voz da IA." },
      { ttl: "Superfícies", sub: "Creme quente, não cinza frio." },
      { ttl: "Texto", sub: "Tinta quente — nunca preto puro." },
      { ttl: "Semântica", sub: "Teal (sucesso) · Amber (rascunho) · Red (erro)." },
    ],
  },
  {
    num: "04",
    title: "",
    titleEm: "Espaço & Forma",
    count: "3 itens",
    grid: "grid3",
    cards: [
      { ttl: "Espaçamento", sub: "Escala base-4." },
      { ttl: "Raios", sub: "4 · 8 · 12 · 16 · pill." },
      { ttl: "Sombras", sub: "3 elevações · foco indigo." },
    ],
  },
  {
    num: "05",
    title: "",
    titleEm: "Componentes",
    count: "7 itens",
    grid: "grid3",
    cards: [
      { ttl: "Botões", sub: "Primário · Secundário · Ghost · Ícone · AI." },
      { ttl: "Inputs", sub: "Campos, textarea, checkbox, toggle." },
      { ttl: "Badges", sub: "Status, idiomas, rascunho." },
      { ttl: "Avatares", sub: "Iniciais, grupo, presença." },
      { ttl: "Dropdown & Select", sub: "Menus e seletores." },
      { ttl: "Cards & Modal", sub: "Contêineres e overlays." },
      { ttl: "Toasts & Callouts", sub: "Feedback e avisos inline." },
    ],
  },
  {
    num: "06",
    title: "",
    titleEm: "Navegação",
    count: "3 itens",
    grid: "grid3",
    cards: [
      { ttl: "Sidebar", sub: "Árvore de coleções, status MCP." },
      { ttl: "Top bar", sub: "Breadcrumb, ações, idioma." },
      { ttl: "Command palette", sub: "⌘K — busca e ações rápidas." },
    ],
  },
  {
    num: "07",
    title: "assinatura",
    titleEm: "Padrões",
    count: "3 itens",
    grid: "grid3",
    cards: [
      { ttl: "Check-in da IA", sub: "O gesto assinatura: \u201cposso continuar?\u201d" },
      { ttl: "Linha de página", sub: "Densidade de lista com idiomas e status." },
      { ttl: "Estado vazio", sub: "Convite à conversa, não à escrita." },
    ],
  },
  {
    num: "08",
    title: "completas",
    titleEm: "Composições",
    count: "4 telas",
    grid: "grid2",
    cards: [
      { ttl: "Editor — 5 estados", sub: "Em branco · escrevendo · check-in · foco · pronto para publicar." },
      { ttl: "Dashboard", sub: "Lista de páginas de uma coleção." },
      { ttl: "Login", sub: "Entrada, com estado de erro." },
      { ttl: "Setup wizard", sub: "Onboarding — conexão com modelo de IA." },
    ],
  },
];

const principles = [
  {
    title: "Uma conversa, não um formulário.",
    body: "A IA pergunta; a pessoa responde. O documento é subproduto do diálogo — nunca o oposto.",
  },
  {
    title: "Mostre o invisível.",
    body: "Quando a IA está lendo, checando ou escrevendo, isso precisa ser visível — sem ruído, sem \u201ccarregando...\u201d genérico.",
  },
  {
    title: "Historiador, não ghostwriter.",
    body: "A IA registra o que a equipe já sabe. O tom é sóbrio, curioso, atento — nunca performático.",
  },
  {
    title: "Português primeiro.",
    body: "Todo copy nasce em pt-BR. Traduções vêm depois, rastreadas e visíveis.",
  },
];

function BookMark({ size = 26 }: { size?: number }) {
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

function Arrow() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
      <path d="M7 17 17 7M7 7h10v10" />
    </svg>
  );
}

export default function DesignSystemPage() {
  return (
    <div className={styles.wrap}>
      <div className={styles.header}>
        <BookMark />
        Historiador Doc
      </div>

      <div className={styles.hero}>
        <div className={styles.eyebrow}>Nexian Tech · Design System v0.1</div>
        <h1 className={styles.heroTitle}>
          Documentação
          <br />
          <em>que conversa</em> de volta.
        </h1>
        <div className={styles.lede}>
          Um sistema de design para a plataforma onde a memória da equipe vira página — conversando
          com a IA, em vez de escrevendo sozinho.
        </div>

        <div className={styles.meta}>
          <div className={styles.metaCell}>
            <div className={styles.metaKey}>Stack</div>
            <div className={styles.metaVal}>Inter · Instrument Serif · JetBrains Mono</div>
          </div>
          <div className={styles.metaCell}>
            <div className={styles.metaKey}>Primária</div>
            <div className={styles.metaVal}>
              <strong>Indigo 600</strong> · #4F46E5
            </div>
          </div>
          <div className={styles.metaCell}>
            <div className={styles.metaKey}>Ícones</div>
            <div className={styles.metaVal}>Lucide · traço 2px</div>
          </div>
          <div className={styles.metaCell}>
            <div className={styles.metaKey}>Idioma base</div>
            <div className={styles.metaVal}>Português (BR)</div>
          </div>
        </div>
      </div>

      <div className={styles.section}>
        <div className={styles.sectionHead}>
          <span className={styles.sectionNum}>00</span>
          <span className={styles.sectionTitle}>
            <em>Princípios</em> de produto
          </span>
        </div>
        <div className={styles.principlesGrid}>
          {principles.map((p) => (
            <div key={p.title} className={styles.principle}>
              <div className={styles.principleTitle}>{p.title}</div>
              <div className={styles.principleBody}>{p.body}</div>
            </div>
          ))}
        </div>
      </div>

      {sections.map((section) => (
        <div key={section.num} className={styles.section}>
          <div className={styles.sectionHead}>
            <span className={styles.sectionNum}>{section.num}</span>
            <span className={styles.sectionTitle}>
              <em>{section.titleEm}</em>
              {section.title && ` ${section.title}`}
            </span>
            {section.count && <span className={styles.sectionCount}>{section.count}</span>}
          </div>
          <div className={section.grid === "grid2" ? styles.grid2 : styles.grid3}>
            {section.cards.map((c) => (
              <span key={c.ttl} className={styles.card}>
                <span className={styles.cardTitle}>
                  {c.ttl}
                  <Arrow />
                </span>
                <span className={styles.cardSub}>{c.sub}</span>
              </span>
            ))}
          </div>
        </div>
      ))}

      <div className={styles.foot}>
        <span>historiador-doc · design system</span>
        <span>v0.1 · abr 2026</span>
      </div>
    </div>
  );
}
