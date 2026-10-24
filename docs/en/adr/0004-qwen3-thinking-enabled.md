# ADR 0004 — Qwen3 keeps its thinking enabled, no `/nothink` suffix

**English** | [Français](../../fr/adr/0004-qwen3-reflexion-activee.md)

- Status: accepted
- Date: 2026-10-24

## Context

The correction prompt appended ` /nothink` to the user's message. Qwen3 does not recognize that command (the real one is `/no_think`): the model thought anyway, and sometimes copied `/nothink` into the corrected text, which would then be typed into the document. The official way to turn thinking off is an empty `<think></think>` block at the start of the answer.

## Decision

Measured on four French and English sentences with the production prompt:

| Mode | Result |
|------|--------|
| Empty `<think>` block (thinking off) | text returned unchanged almost every time (35 to 260 ms) |
| Thinking on | capitals, accents and punctuation fixed (~0.5 s on GPU) |

Decision: thinking stays on, no suffix, and the `<think>…</think>` block is removed from the output. Budget of 1024 tokens; if the thinking does not finish, the output is empty.

## Consequences

- Clearly better corrections, at the price of 0.5 s on GPU (several seconds on CPU).
- On a long text the small model can loop in its thinking ("The user's name is "nous" which is correct" repeated) and never answer. An empty output must never erase the dictation: see ADR 0005.
- A larger model (Qwen3-1.7B) is still to be tried.
