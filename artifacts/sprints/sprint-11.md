# Sprint 11 — Historiador

**Theme:** From robotic writer to collaborative documentation partner
**Sprint Goal:** Replace the one-shot "AI dumps a wall of markdown" experience with a conversational, agentic flow where the AI asks before it writes, edits the canvas with visible diffs, and lets the user pick how autonomous the agent is on each task.

---

## Why this sprint

Today's Historiador has the pieces but the experience is broken in three specific ways the team has heard from users and confirmed internally:

1. **No discovery conversation.** The AI takes the user's first prompt and generates an entire document — no clarifying questions about audience, purpose, scope, or structure.
2. **Canvas writes are destructive.** The AI replaces everything on the canvas instead of making targeted edits, showing diffs, or writing at the cursor.
3. **Pacing is wrong.** The AI dumps giant markdown blocks in one pass, so the user can't steer mid-draft.

Historiador's promise is the *democratization of documentation* — anyone can create brilliant docs. That promise breaks if the AI feels like a vending machine. Sprint 11 makes the AI behave like a thoughtful collaborator: it asks, it proposes, it drafts in pieces, and it edits the canvas surgically.

---

## Sprint objectives (what must be true at sprint end)

1. When a user opens a new doc and types a goal into chat, the AI **asks at least one relevant clarifying question** before writing anything on the canvas (unless the user explicitly opts out).
2. Users can choose **one of three autonomy modes** per task: *Propose*, *Checkpointed draft*, or *Autonomous draft*. The selector is visible and persistent.
3. The AI writes to the canvas using **block-level operations** (insert, replace, append, delete) — never a full-canvas overwrite. Every write is **diffable and undoable**.
4. The AI's chat voice is **warm, concise, and human** — driven by a versioned agent prompt, not hard-coded strings.
5. The agent is **canvas-aware**: it reads the current doc state, the user's selection, and cursor position before responding.
6. Users can take a doc from a blank canvas to an **exported, shareable deliverable** in one continuous flow — with the AI supporting each stage (plan → write → review → publish).

---

## User Stories

Stories are grouped by the four documentation journey stages. Each story names the user persona ("Author" = anyone using Historiador — Historiador's user base is intentionally broad), describes the value, and lists acceptance criteria in testable form.

---

### Stage 1 — Discovery & Planning

> *"Before I write your doc, let me understand what you actually need."*

**US-11.01 — Conversational intake**
*As an Author, I want the AI to ask me clarifying questions about my document before it starts writing, so the output matches my intent instead of a guess.*

Priority: **P0**

Acceptance criteria:
- When the Author sends the first message describing a doc they want, the AI responds with **2–4 clarifying questions** covering at least: audience, purpose, desired length/depth, and known constraints.
- The AI does **not** write to the canvas on the first turn unless the user explicitly says something like "just write it" or "skip questions".
- Questions are adaptive: if the user already included the audience in their prompt, the AI does not ask again.
- Skipping discovery is a one-click option in the chat ("Skip planning, just draft it").
- The conversation summary (answers to discovery questions) is attached to the doc metadata so it persists across sessions.

---

**US-11.02 — Collaborative outline builder**
*As an Author, I want the AI to propose a document outline I can edit before any prose is written, so I control structure without having to rewrite full sections later.*

Priority: **P0**

Acceptance criteria:
- After discovery, the AI proposes an outline (H2/H3 headings + one-line purpose per section) in chat *before* writing to the canvas.
- The Author can edit the outline inline in chat (add/remove/reorder/rename sections) and reply "looks good" or click **Accept outline**.
- Once accepted, the outline is placed on the canvas as headings with empty body sections, ready to be filled in.
- The outline is stored separately from the canvas content so the agent can reference it during authoring ("we are working on Section 3 of the outline").

---

**US-11.03 — Template & starting-point picker**
*As an Author who doesn't know where to start, I want the AI to suggest a starting template based on my description, so I'm not staring at a blank canvas.*

Priority: **P1**

Acceptance criteria:
- When intake signals a known doc type (how-to, runbook, RFC, meeting notes, knowledge article, etc.), the AI offers 1–3 templates inline.
- Each suggestion shows title, one-line description, and a preview pane.
- Choosing a template seeds the canvas with a structured skeleton; the user can still modify it via outline builder.

---

### Stage 2 — Authoring & Editing

> *"Let me write alongside you, not at you."*

**US-11.04 — Humanized agent persona**
*As an Author, I want the AI to talk to me like a thoughtful colleague, not a form letter, so the collaboration feels natural.*

Priority: **P0**

Acceptance criteria:
- A versioned system prompt defines the agent's persona: warm, concise, inquisitive, plain-spoken. Lives in `/prompts/agent/v{N}.md` with changelog.
- The AI avoids filler openings ("Certainly! Here is...", "I'd be happy to..."), robotic closings, and excessive hedging.
- The AI uses the Author's wording when possible (if the user says "runbook", the AI says "runbook", not "operational documentation artifact").
- The agent signs off on long actions with a one-line summary of what it just did ("Filled in Section 2 with the auth flow — want me to keep going?") rather than silence or a form acknowledgment.
- An internal "voice eval" set of 20 sample prompts returns responses scored ≥ 8/10 on warmth & concision by product review.

---

**US-11.05 — Autonomy selector per task**
*As an Author, I want to choose how autonomous the agent is for a given request, so I can trust it with small edits and keep control over big ones.*

Priority: **P0**

Acceptance criteria:
- A selector in the chat composer offers three modes:
  - **Propose** — agent describes what it will do, user approves each step.
  - **Checkpointed draft** — agent proposes a plan once, then drafts through it pausing at section boundaries.
  - **Autonomous draft** — agent drafts end-to-end; user reviews diffs after.
- The selected mode is visible on the message bubble ("Drafted autonomously — review diff").
- The default mode is **Checkpointed** for new docs; **Propose** for edits on existing docs.
- The mode persists per conversation but can be changed per-turn.

---

**US-11.06 — Canvas write primitives (block-level ops)**
*As an Author, I want the AI to edit specific parts of my doc instead of replacing the whole canvas, so I never lose work or context.*

Priority: **P0** (technical enabler — blocks US-11.07 and review stories)

Acceptance criteria:
- Canvas exposes typed operations the agent can call: `insertBlock(afterId, block)`, `replaceBlock(id, block)`, `appendToSection(headingId, blocks)`, `deleteBlock(id)`, `moveBlock(id, newParentId, index)`.
- Every operation returns a `changeId` and is reversible via `undo(changeId)`.
- The agent is **prohibited** by tool design from issuing a "replace entire canvas" call. Full rewrites must be expressed as a sequence of block ops.
- All agent-originated ops emit an event stream consumed by the diff UI (US-11.07).
- Operations are pure and deterministic given canvas state + op payload, enabling replay and testing.

---

**US-11.07 — Visible diffs with accept/reject**
*As an Author, I want to see exactly what the AI changed on my canvas and accept or reject each change, so I never lose work to an overconfident AI.*

Priority: **P0**

Acceptance criteria:
- Each agent-originated change renders as a block-level diff (added blocks in green, replaced blocks show before/after, deleted blocks struck through).
- Each diff has **Accept** and **Reject** controls. Rejecting restores prior state.
- A "batch" of diffs from a single agent turn can be accepted or rejected as a group.
- Changes are not persisted to the saved doc until accepted (or until autonomous mode flag is on — in which case they're persisted with an undoable history entry).
- Keyboard: `Enter` to accept focused diff, `Esc` to reject.

---

**US-11.08 — Section-by-section drafting (pacing)**
*As an Author, I want the AI to draft one section at a time rather than dumping the entire document at once, so I can steer as it writes.*

Priority: **P0**

Acceptance criteria:
- In Checkpointed mode, the agent drafts one outline section, writes it to the canvas via block ops, then pauses with a one-line summary and "Continue" / "Revise this section" / "Skip" buttons.
- In Propose mode, the agent shows a preview of the section in chat before writing.
- In Autonomous mode, the agent drafts all sections but still emits separate diff batches per section (not one giant batch).
- A progress indicator shows "Section 2 of 5 — Authorization flow".

---

**US-11.09 — Canvas-aware context**
*As an Author, I want the AI to actually read what's already on the canvas and respect what I've written, so it stops contradicting or overwriting my work.*

Priority: **P0**

Acceptance criteria:
- The agent receives: current canvas content (markdown), current selection (if any), cursor block id, outline state, and conversation history every turn.
- When the user says "rewrite this" with a selection, the agent targets only the selected blocks.
- When the user references "Section 2" or "the intro", the agent resolves the reference against the current outline and operates on the correct blocks.
- The agent cites which blocks it is about to modify before modifying them ("I'll rewrite the Setup section — 3 blocks").

---

**US-11.10 — Inline AI on selection**
*As an Author, I want to select text and ask the AI to rewrite, expand, shorten, or fix it, without leaving the canvas for the chat panel.*

Priority: **P1**

Acceptance criteria:
- Selecting text on the canvas surfaces a floating AI toolbar: **Rewrite**, **Expand**, **Shorten**, **Fix grammar**, **Ask…** (custom).
- All inline actions go through the same agent prompt and canvas-op primitives as chat.
- Inline actions produce a diff (per US-11.07), not a silent overwrite.

---

### Stage 3 — Review & Collaboration

> *"Help me make this better before anyone else sees it."*

**US-11.11 — AI review pass**
*As an Author, I want to ask the AI for a review of my document and get specific, in-context feedback instead of a generic rewrite.*

Priority: **P1**

Acceptance criteria:
- A "Review this doc" action in the chat runs the AI in review mode: it leaves **inline comments** on specific blocks, does **not** edit the canvas.
- Each comment has a type: *missing info*, *unclear*, *tone*, *factual check*, *suggestion*.
- Comments can be resolved or converted into an edit via "Apply suggestion" (which then goes through the diff flow).
- The chat summary lists the top 3 things to fix, ordered by impact.

---

**US-11.12 — Suggestion mode (non-destructive edits)**
*As an Author, I want to ask the AI for changes that appear as suggestions — not direct edits — so I can review them the same way I'd review a teammate's PR.*

Priority: **P1**

Acceptance criteria:
- A "Suggest edits" toggle in the autonomy selector forces the agent to emit `suggestBlockChange` ops instead of `replaceBlock`.
- Suggestions render as tracked changes with an inline "Accept" / "Reject" chip next to each.
- Suggestions persist across sessions until acted on.

---

### Stage 4 — Publishing & Sharing

> *"Ship it with confidence."*

**US-11.13 — Pre-publish readiness check**
*As an Author, I want the AI to check my doc for completeness and quality before I publish, so I don't ship something half-finished.*

Priority: **P1**

Acceptance criteria:
- A "Ready to publish?" action runs checks: broken internal references, empty sections, TODOs/placeholders left in the text, headings hierarchy issues, tone consistency.
- Output is a short checklist: ✅ / ⚠️ per item with jump-to-block links.
- The Author can fix issues one-by-one (each fix flows through the diff UI) or dismiss.

---

**US-11.14 — Export with AI-generated summary**
*As an Author, I want to export my doc (MD / PDF / HTML) with an auto-generated TL;DR and share link, so readers know what they're getting before they read.*

Priority: **P1**

Acceptance criteria:
- Export menu offers MD, PDF, HTML with a "Generate summary" toggle (default on).
- The summary is a ≤120-word TL;DR prepended to the exported file and shown on the share page.
- The summary is generated fresh each export and editable before export.

---

**US-11.15 — Shareable link with read analytics**
*As an Author, I want a shareable link to my published doc so I can send it to readers without exporting a file.*

Priority: **P2** (stretch)

Acceptance criteria:
- "Share link" produces a public/internal URL based on permissions.
- Link preview (OG tags) shows title + AI-generated summary.
- Simple analytics: views, unique readers, average read-through %.

---

### Cross-cutting enablers

**US-11.16 — Agent prompt + tool schema infrastructure**
*As a developer, I need a clean separation between the agent's persona prompt, its tool definitions, and its runtime so we can iterate on voice and behavior without redeploying the editor.*

Priority: **P0** (technical)

Acceptance criteria:
- Agent prompt is loaded from `/prompts/agent/` at runtime; version is logged with every request.
- Tool schemas (canvas ops, review ops, export ops) are declared once and consumed by both the agent and the UI.
- A feature flag can switch between prompt versions for A/B comparison.
- Prompt changes do not require a frontend deploy.

---

**US-11.17 — Observability & voice regression suite**
*As the team, we need to know when the AI's behavior regresses on tone, pacing, or canvas-op correctness before users do.*

Priority: **P1** (technical)

Acceptance criteria:
- An eval set of ~30 scripted user turns runs against the agent nightly.
- Assertions include: "did not overwrite the canvas", "asked ≥1 clarifying question on new-doc prompts", "no robotic opener", "respected selected blocks", "stayed on outline".
- Regressions page the on-call PM + dev before release.

---

## Priority roll-up

| P0 — Must ship | P1 — Should ship | P2 — Stretch |
|---|---|---|
| US-11.01 Conversational intake | US-11.03 Template picker | US-11.15 Share link + analytics |
| US-11.02 Outline builder | US-11.10 Inline AI on selection | |
| US-11.04 Humanized persona | US-11.11 AI review pass | |
| US-11.05 Autonomy selector | US-11.12 Suggestion mode | |
| US-11.06 Canvas write primitives | US-11.13 Pre-publish check | |
| US-11.07 Visible diffs | US-11.14 Export with summary | |
| US-11.08 Section-by-section drafting | US-11.17 Voice regression suite | |
| US-11.09 Canvas-aware context | | |
| US-11.16 Agent prompt infra | | |

**Cut-line principle:** If a P0 is at risk, cut any P1. Ship the full P0 set or the sprint goal fails — each P0 blocks part of the core loop (intake → outline → draft → diff → accept).

---

## Dependency map

```
US-11.16 (agent infra) ──┬──> US-11.04 (persona)
                         │
                         └──> US-11.09 (canvas-aware)
                                   │
US-11.06 (canvas ops) ────────────┤
                                   │
                                   ├──> US-11.07 (diffs)
                                   │        │
                                   │        └──> US-11.08 (section drafting)
                                   │                 │
US-11.01 (intake) ──> US-11.02 ──> US-11.05 ────────┘
                     (outline)    (autonomy modes)
```

Read: the agent-infra + canvas-op primitives (US-11.06, 11.16) need to land early in the sprint because everything else depends on them.

---

## Risks & mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Agent prompt tuning takes longer than expected | Humanization story slips, sprint goal weakened | Treat the prompt as code: version it, review it, test it against the eval set. Timebox iteration. |
| Block-level diff UI is more complex than anticipated | Blocks every authoring story | Start with the simplest possible diff (per-block green/red) before pursuing intra-block word diffs. |
| Canvas state model isn't structured enough for block ops | Forces a mid-sprint refactor of the data model | Confirm canvas block schema in sprint kickoff — if insufficient, raise the refactor as the first task. |
| Agent chooses wrong autonomy mode or overrides user | User loses trust in the selector | Log every autonomy decision. Default conservatively (Checkpointed / Propose). Make the current mode visually obvious in the UI. |
| Regressions in AI voice as prompts evolve | "Humanized" becomes "humanized then robotic again" | US-11.17 (nightly eval) is a P1, not a P2 — don't drop it. |
| Full journey coverage (4 stages) is too broad | Sprint slips across the board | Review & Publishing stories are mostly P1. The P0 critical path is concentrated in Discovery + Authoring. |

---

## Definition of Done

A story is Done when:
- Code is merged to main and deployed behind a feature flag.
- Acceptance criteria pass in staging against the eval set (where applicable).
- Telemetry for the new surface is live (which mode used, accept vs. reject rate, discovery-questions count per session).
- PM + design have signed off on copy and UI.
- The change is covered by the voice regression suite if it touches agent behavior.
- Docs: a one-pager on how to use the new flow lives in Historiador's own product Help section — written *using Historiador*.

---

## Demo storyline (sprint review)

A live walkthrough of the new experience, end-to-end:
1. New user opens a blank Historiador doc and types *"help me write a runbook for our database failover"*.
2. AI **asks 3 clarifying questions** (audience, systems, what should be in vs. out of scope).
3. AI **proposes an outline**; user edits one heading, accepts.
4. User picks **Checkpointed** autonomy. Agent drafts Section 1, writes it to canvas as a **diff**; user accepts.
5. User selects a paragraph, clicks **Rewrite — more concise**. Sees the diff, accepts.
6. User clicks **Review this doc**. AI leaves 4 inline comments; user resolves 2, applies 1 as an edit.
7. User clicks **Ready to publish?** — all green. Clicks **Export → PDF with summary**. Downloads.
8. Throughout: chat tone is warm, nothing on the canvas was ever overwritten without the user seeing a diff.

Any demo step that doesn't work is a sprint miss.

---

## Success metrics (tracked for 2 weeks post-sprint)

- **Discovery rate**: % of new-doc sessions where the AI asks at least one clarifying question → target **≥ 90%** (excluding explicit skip).
- **Diff accept rate**: % of agent-originated diffs accepted by users → target **≥ 70%** (low rates mean the agent is writing wrong things).
- **Canvas-overwrite events**: full-canvas replacements → target **0**.
- **Avg. section-level drafting cycles per doc**: checkpoints hit before completion → target **≥ 3** (shows pacing is working).
- **Voice eval score**: internal rubric on 30 prompts → target **≥ 8/10**.
- **Qualitative**: 5 user interviews post-sprint — does the AI feel "human"? Open-coded themes.

---

## Key dates (placeholder — team to fill in)

| Date | Event |
|---|---|
| TBD | Sprint kickoff + canvas schema check |
| +3 days | Agent infra (US-11.16) + canvas ops (US-11.06) merged |
| Mid-sprint | Checkpoint demo: intake → outline → first diff accepted |
| -2 days | Feature-flag-on dry run across the team |
| Sprint end | Demo + retro |

---

## Out of scope for Sprint 11

Called out explicitly so it doesn't leak in:
- Multi-user real-time collaboration on the canvas (comments, presence).
- Permissions/roles model beyond what already exists.
- Mobile editor experience.
- Full i18n of the agent persona (voice is tuned for the current primary language first; localization follows).
- Training a custom model — we're solely changing prompting, tools, and UX.
- Version history UI beyond undo of agent changes.

If these come up mid-sprint, they go to the Sprint 12 candidate list, not into Sprint 11.
