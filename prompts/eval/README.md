# Voice regression eval suite (Sprint 11, phase B8 / US-11.17)

This directory ships scripted prompts that exercise the Sprint 11
agent across the behaviors we care about:

- **tone** — warm, concise, plain-spoken; no form-letter openings
- **intake gating** — blank canvas should yield 2–4 clarifying
  questions, not a markdown dump
- **canvas-op correctness** — the agent never calls a tool that
  would overwrite the whole canvas (US-11.06)
- **discovery-question presence** — ≥90% of blank-page turns ask
  ≥1 question

Each prompt file is a YAML-front-mattered markdown file. Assertions
are applied post-hoc by the harness (`scripts/voice-eval/run.sh`).
Harness shape:

```
prompts/eval/
  ├── 001-blank-runbook.md       # intake + tone
  ├── 002-mid-draft-rewrite.md   # replace_block correctness
  ├── 003-long-selection.md      # selection carried through
  └── ...
```

Each file looks like:

```
---
id: 001-blank-runbook
tags: [intake, tone]
canvas: ""
selection: null
user_turn: "help me write a runbook for database failover"
assert:
  - contains_question: true
  - no_form_letter: true
  - no_canvas_overwrite: true
---

(notes for humans — never fed to the model)
```

The eval harness runs each file against the stub provider (so CI
does not require real LLM credentials), parses the recorded
turns, and fails the run when any assertion fails. Regressions
page on-call via the existing GitHub Actions alert wiring.
