# ADR-008: Conversational Split-Pane Editor Architecture

**Status:** Accepted
**Date:** 2026-04-12
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

The original PRD spec described the AI editor as a prompt-and-response loop: the author submits a brief, receives a full draft, and sends follow-up messages to iterate. This is functional but treats the AI as a text generator rather than a collaborator.

The revised direction is a **split-pane conversational editor**:
- **Left pane:** a persistent conversation thread between the author and the AI
- **Right pane:** the documentation page, building in real-time as the conversation resolves what needs to be written

The conversation is the process. The page is the artifact. The AI doesn't wait for a complete brief — it asks, listens, generates sections as clarity emerges, and adapts when the author redirects. The author can see the document forming and course-correct based on what they see, without ever leaving the conversation.

This model was chosen because it matches how documentation knowledge is actually extracted from people — through dialogue, not forms. It also means the editor experience is itself a demonstration of the product's value proposition.

---

## Decision

**Implement the AI editor as a split-pane interface** where:

- The left pane is a WebSocket-connected conversation thread
- The right pane is a live markdown preview, updated via streaming as the AI generates content
- The AI operates in two distinct modes: **conversation mode** (asking, clarifying, acknowledging) and **generation mode** (writing page sections, streamed to the right pane)
- The conversation is persisted to the `editor-conversations` Chronik topic as a durable event stream
- The author can click any section in the right pane to trigger a targeted follow-up in the conversation

---

## UI Layout

```
┌─────────────────────────────────────────────────────────────────┐
│  [Page title — editable]                          [Draft] [Publish] │
├───────────────────────────┬─────────────────────────────────────┤
│                           │                                     │
│   CONVERSATION            │   LIVE PAGE PREVIEW                 │
│                           │                                     │
│  AI: What do you need     │  # Employee Onboarding              │
│      to document?         │                                     │
│                           │  ## Before Day One                  │
│  You: The employee        │  Send the new hire a welcome        │
│       onboarding process  │  email with their start date,       │
│                           │  laptop delivery info, and          │
│  AI: Who's the primary    │  first-day schedule...              │
│      reader — new hire    │                                     │
│      or the HR team?      │  ## Day One ░░░░░░░░░░             │
│                           │  [streaming...]                     │
│  You: The new hire        │                                     │
│                           │                                     │
│  [Type a message...]  [→] │                                     │
└───────────────────────────┴─────────────────────────────────────┘
```

The right pane shows content as it streams — sections appear word by word, with a visible cursor indicating generation in progress. Completed sections are visually distinct from in-progress ones. The author never needs to look at raw markdown unless they choose to.

---

## AI Behaviour Model

The AI operates in two distinct modes, determined by the system prompt and conversation state:

### Conversation Mode
The AI asks questions, seeks clarification, and acknowledges author input. Responses appear only in the left pane. No page content is generated.

Triggers:
- Ambiguous or incomplete brief
- Author has redirected or expressed dissatisfaction
- After completing a section, checking in before continuing
- Author's message is clearly a question or feedback, not a content prompt

Example conversation-mode responses:
- *"Who's the primary reader — a new employee or the HR team running the process?"*
- *"Should I include a section on IT access setup, or is that handled separately?"*
- *"Got it — I'll cut the intro and start with the steps. Should I keep the section on exceptions?"*

### Generation Mode
The AI writes page content. Each chunk of generated text is streamed to the right pane in real-time. The left pane shows a subtle indicator ("Writing...") while generation is active.

Triggers:
- Sufficient clarity has been established through conversation
- Author explicitly requests content: *"Ok, write the first section"*
- Author approves a generated section: *"Looks good, keep going"*

The AI never generates a complete page in one shot. It generates one section at a time, pausing to check in with the author after each major section before continuing.

---

## Technical Architecture

### WebSocket Connection
The editor maintains a persistent WebSocket connection between the Next.js frontend and the Axum API server. The connection handles:
- Sending author messages
- Receiving AI conversation responses (left pane)
- Receiving AI-generated content streams (right pane)

A message envelope distinguishes conversation from generation:
```typescript
type EditorMessage =
  | { type: 'conversation'; content: string }       // → left pane
  | { type: 'generation_chunk'; content: string }   // → right pane (streamed)
  | { type: 'generation_complete'; section: string } // → section locked in right pane
  | { type: 'ai_thinking' }                          // → "Writing..." indicator
```

### Streaming Architecture
LLM responses stream token by token from the provider (OpenAI/Anthropic) through the Axum WebSocket handler to the frontend. The backend does not buffer the full response before sending — it pipes the stream directly to the WebSocket. This gives sub-second first-token latency on the right pane.

```
Author types → WebSocket → Axum handler
                                ↓
                         LLM API (streaming)
                                ↓
              token by token → WebSocket → Frontend
                                              ↓
                              {type: conversation}  → Left pane
                              {type: generation_chunk} → Right pane (live)
```

### Chronik-Stream Persistence
Every message in the conversation — author turns, AI turns, generation events — is written to the `editor-conversations` Chronik topic as an ordered event stream. The topic key is the `page_id`, so all events for a given page are co-partitioned and queryable together.

```
{ page_id, timestamp, role: 'author' | 'ai', mode: 'conversation' | 'generation', content }
```

This means:
- The conversation is durable across browser sessions — authors can return to an in-progress page
- The full editorial history of a document is preserved
- Abandoned conversations (page never published) are visible as a signal for gap detection

### Section Click-to-Edit
When the author clicks a section in the right pane, the frontend sends a targeted context message to the conversation:
```
{ type: 'section_focus', section_heading: "Day One", current_content: "..." }
```
The AI acknowledges in the left pane and enters a focused editing mode for that section, treating subsequent messages as edits to that specific section rather than new content.

### Draft Persistence
The right pane content is auto-saved to PostgreSQL as a `page_version` record with status `draft` every 30 seconds, and on every `generation_complete` event. The author never loses work if they close the tab.

---

## Options Considered

### Option A: Split-Pane with Real-Time Streaming ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Author experience | High — see the document forming as you talk |
| AI collaboration quality | High — author course-corrects in real-time |
| Implementation complexity | High — WebSockets + streaming + two-pane sync |
| Frontend complexity | High — streaming markdown renderer, section state |
| Backend complexity | Medium — pipe LLM stream to WebSocket |

**Chosen because** the feedback loop between conversation and document is the core design idea. Without real-time rendering, the author is working blind — they can't redirect the AI based on what they see.

### Option B: Sequential (Chat First, Document Delivered)

Conversation happens entirely in the left pane. When the AI judges the document is ready (or the author requests it), the full page is rendered in the right pane.

**Rejected** — breaks the core feedback loop. The author can't redirect the AI based on what the page looks like because they can't see it forming. Retrieves the "prompt and receive" dynamic we were trying to move away from.

### Option C: Inline Editor (No Split Pane)

The AI writes directly into a single editor. Conversation is embedded as comments or a sidebar. The document IS the interface.

**Rejected for v1** — cognitively confusing to mix AI conversation with authored content in the same surface. The split-pane separation is a cleaner mental model: left is dialogue, right is output.

---

## Consequences

**Easier:**
- Authors never stare at a blank page — the AI asks the first question
- Course correction is immediate — the author sees the document forming and can redirect mid-section
- The conversation history in Chronik makes "resume where I left off" a natural feature, not a build
- Section-level editing maintains document coherence — the AI knows the full context of all sections

**Harder:**
- WebSocket connection management adds complexity — reconnection, heartbeat, message ordering
- The streaming markdown renderer must handle partial markdown gracefully (a heading mid-stream before the content exists)
- The AI system prompt must reliably distinguish conversation mode from generation mode — prompt engineering is load-bearing here
- Two-pane layout requires careful responsive design for smaller screens

**Must revisit:**
- If streaming from OpenAI/Anthropic introduces latency spikes, evaluate buffering at the section boundary (buffer one paragraph before streaming to the UI) to smooth the visual experience
- Section-level locking (preventing the AI from editing a section the author has approved) needs a clear UX treatment — not designed in v1, but the data model should support it
- Mobile experience is explicitly out of scope for v1 — the split-pane layout requires a minimum viewport width

---

## Action Items

1. [ ] Implement WebSocket handler in `crates/api` for the editor connection
2. [ ] Define the `EditorMessage` envelope type in `packages/types`
3. [ ] Implement LLM stream → WebSocket pipe in the Axum handler (no buffering)
4. [ ] Write the AI system prompt distinguishing conversation mode from generation mode — validate against 10 test briefs before Sprint 4 ships
5. [ ] Implement streaming markdown renderer in Next.js (handle partial markdown gracefully)
6. [ ] Implement section click-to-edit: `section_focus` message type + focused editing mode in the AI
7. [ ] Implement auto-save of right pane content to `page_versions` every 30 seconds
8. [ ] Write Chronik event schema for `editor-conversations` topic
9. [ ] Define minimum viewport width for the split-pane layout; design fallback for narrower viewports
