---
id: 002-mid-draft-rewrite
tags: [canvas_ops, rewrite]
canvas: |
  <!-- block:01960000-0000-7000-8000-000000000001 -->

  # Database failover

  <!-- block:01960000-0000-7000-8000-000000000002 -->

  The failover procedure is executed when the primary database becomes unavailable for more than 30 seconds.
selection: "The failover procedure is executed when the primary database becomes unavailable for more than 30 seconds."
user_turn: "make this shorter"
assert:
  - uses_replace_or_suggest: true
  - targets_block: "01960000-0000-7000-8000-000000000002"
  - no_canvas_overwrite: true
---

The agent should emit a `replace_block` or `suggest_block_change`
on the exact block carrying the selection. Any tool call that
omits `block_id` or targets a different block is a regression.
