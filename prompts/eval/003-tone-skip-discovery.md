---
id: 003-tone-skip-discovery
tags: [tone, skip_discovery]
canvas: ""
selection: null
user_turn: "help me write a runbook for database failover"
skip_discovery: true
assert:
  - contains_question: false
  - no_form_letter: true
  - proposes_outline: true
---

After Skip Discovery the agent jumps directly to outline
proposal — no additional clarifying questions. Tone should still
be warm-professional and the outline should contain concrete
section headings, not placeholders.
