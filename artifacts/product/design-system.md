# Historiador Doc — Design System

**Historiador Doc** is an AI-powered documentation platform by **Nexian Tech** (Brazil). Its thesis: sharing knowledge should feel like a conversation between two people. The product uses an AI-mediated split-pane editor where one side is a chat, the other is the rendered document — you talk, the doc writes itself.

The name "Historiador" is Portuguese for **historian** — someone who records and preserves knowledge. That framing anchors the whole system: this is a tool for people who treat documentation as a craft, not a chore.

This repo is the single source of truth for the product's visual language: tokens, components, patterns, and the Editor UI kit.

## Source material

- **Primary brief**: `uploads/historiador-figma-design-plan.docx.pdf` — the original Figma library plan authored by Gabriel Specian (April 2026). All token names, component inventory, and screen specs derive from it.
- No codebase or Figma file was provided yet; this is a **net-new** system. Tokens match the plan's hex values where specified; everything else is original to this document.


## Content fundamentals

**Voice**: friendly-professional. Closer to Linear/Notion than to Stripe docs. The product talks to the user like a thoughtful colleague who respects their time. Never cute, never stiff.

**Casing**: Sentence case for all UI copy (`New page`, not `New Page`). Labels use ALL CAPS sparingly — only for section dividers (`COLLECTIONS`, `PAGES`) where visual hierarchy demands it.

**Person**: Second-person when addressing the user (`Your workspace is ready.`). First-person singular for the AI assistant (`I'm focused on the Introduction. What would you like to change?`).

**Sample content language**: Portuguese (the primary audience is Brazilian teams). English appears only where the product itself uses English UI chrome (which it largely does — the product is bilingual by default, with page content in PT/EN/ES and UI in whichever the user picks).

**Emoji**: never in UI chrome. A single `✨` sparkle is allowed on AI-affiliated surfaces (Section Check-In, streaming badges) because the plan calls for it and it reads as a functional icon, not decoration.

**Example copy**
- Empty state headline: `Nenhuma página ainda`
- Empty state body: `Comece uma conversa com o assistente — ele vai escrever a página com você, seção por seção.`
- Section check-in: `Introdução concluída — ficou como você imaginou?`
- Button labels: `Continuar escrevendo`, `Refinar esta seção`, `Publicar`

## Visual foundations

**Aesthetic direction: Editorial Historian.** The spec's Indigo-600 + slate-neutral palette is safe startup default. We warm it: the neutrals shift toward cream/ivory, the primary stays deep indigo, and a literary serif (Instrument Serif) shows up for Display-tier moments only. Everything below Display stays in Inter so the UI feels crisp and functional — the serif is an accent, not a theme.

### Colors
- **Primary**: Indigo 600 (`#4F46E5`) as specified. Anchors links, primary buttons, focus rings, active nav.
- **Surfaces**: warmed — Surface/Page is a barely-cream `#FAF8F3` instead of the spec's `#F9FAFB`. Canvas remains pure white so content reads cleanly against a warm page.
- **Text**: ink black `#111827` per spec, but secondary text warms slightly to `#3F3A36` for a paper feel.
- **Semantic**: Amber (drafts), Teal (published / MCP active), Red (missing language / destructive). All kept close to the spec but tuned for warmth.

### Type
- **Display**: Instrument Serif, 48px, used ONLY on empty states and wizard welcome. Italic variant for max character on hero moments.
- **H1–H3, Body, Label, Code**: Inter (400/500/600/700) and JetBrains Mono. Sizes match spec exactly.
- Line heights: 1.2 for display/headings, 1.5 for body, 1.4 for labels.

### Spacing
8px base grid. Tight UI uses 4px increments (e.g., badge internal padding). All margins and paddings are multiples of 4.

### Radii
- 6px — chips, tags, badges (slightly smaller than the spec's radius-md so badges read as pills not cards)
- 8px — buttons, inputs, cards (spec default)
- 12px — modals, command palette panel
- 9999px — pills and avatar circles

### Shadows
Six-tier system matching the spec. Shadow color is tinted warm (`rgba(59, 48, 31, ...)`) instead of pure black so elevation feels like ink on paper rather than CGI.

### Borders
1px Surface/Border (`#E8E2D6`) for all dividers. Inset borders on inputs, outer borders on cards.

### Backgrounds & texture
Flat color only. No gradients, no patterns, no illustrations as background. The warmth comes entirely from the base surface tone. Section headers sometimes use a `Surface/Subtle` band to create visual rhythm without relying on strokes.

### Animation
Minimal and functional. Opacity fades (120ms ease-out), subtle lifts on hover (`translateY(-1px)`), and one signature: a **teal "section-glow" ring** that pulses once when the AI finishes a section. No bouncing, no slide-in decorations.

### Hover / press states
- Hover: `+2% lightness` on filled surfaces; opacity 0.85 on ghost/link elements.
- Press: no shrink; just darker by one token step (`Primary/700` replaces `Primary/600`).
- Focus: outer 4px `Primary/600` ring + inner 2px white ring — a "ring outside the ring" per spec. Danger variants use Red/600.

### Cards
- Default: `Canvas` bg, 1px `Border`, Shadow/SM, 12px radius.
- Padding: 16px (body) / 20px for featured cards.
- Header/footer slots divided by 1px `Border`, 48px tall each.

### Layout rules
- App shell: 240px sidebar + fluid content (1440px design width).
- Content max-width: 720px for reading, 560px for forms, 480px for modals.
- Sticky elements: top bar (56px), floating ToC (right-aligned, 200px from top).

## Iconography

**Primary set: Lucide.** Stroke-based, 2px weight, 24px default (scaled down to 16px or 20px as needed). Chosen because the plan's visual language — thin borders, flat surfaces, no ornament — matches Lucide's restrained style.

- Loaded from CDN: `https://unpkg.com/lucide@latest`
- Also used in the wordmark mark (a stylized open-book glyph derived from Lucide's `book-open` outline).
- Never mix icon sets. Never fall back to emoji (except ✨ on AI affordances — functional, not decorative).
- Never hand-draw SVG icons — Lucide covers every need the plan describes (document, folder, chevron, magnifier, user, settings, plus, x, check, warning-triangle, info-circle, etc.).

## Caveats

- **No real product access**: this system was built from the plan document alone. Screens are hi-fi mocks, not screenshots of a shipping product. When the real product exists, this system should be reconciled against it.
- **No real logo**: the wordmark + mark in `assets/` is original work matching the editorial direction. Replace with the official brand if one exists.
- **Portuguese copy** was written by the designer; a native speaker on the Nexian team should review for tone and idioms.

## How to use this system

1. Import `colors_and_type.css` at the top of any HTML or JSX file.
2. Pull components from `ui_kits/editor/` as live examples.
3. Reference `preview/` cards when discussing tokens or components in review.
4. For new production work, read `SKILL.md` and the `ui_kits/editor/README.md` first.
