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

Baseline intake case. The agent should ask 2–4 clarifying questions
(audience, scope, failover type) rather than dumping markdown on
the first turn.
