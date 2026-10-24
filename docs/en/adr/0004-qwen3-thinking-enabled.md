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
- **A larger model was tried and rejected for now.** Twelve French cases (six short sentences, three long paragraphs, each degraded and already punctuated), on the GPU:

  | Model and mode | Well formed | Empty | Words changed | Latency |
  |----------------|-------------|-------|---------------|---------|
  | 0.6B, thinking (current) | 8/12 | 0 | **0** | ~620 ms |
  | 0.6B, no thinking | 6/12 | 0 | 1 (drops "jeudi") | ~90 ms |
  | 1.7B, thinking | 3/12 | 7 | 2 | ~1.9 s |
  | 1.7B, no thinking | 8/12 | 0 | 2 (adds words) | ~120 ms |

  The 1.7B thinks so long that it exhausts the 1024-token budget and answers nothing. Without thinking it is five times faster for the same count of well-formed sentences but adds words, which breaks "preserve the user's exact words". Rerun with `tests/e2e_llm_eval.rs` before changing the model or the prompt.
