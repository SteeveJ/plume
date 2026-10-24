# ADR 0007 — Heavy compute runs in `spawn_blocking`, never in the async loop

**English** | [Français](../../fr/adr/0007-calcul-hors-boucle-async.md)

- Status: accepted
- Date: 2026-10-24

## Context

`tokio::task::block_in_place` blocked the thread while Whisper or the LLM computed. The dictation loop, which races the transcription against a new key press (`select!`), only saw that press once the computation was over: the first words of the next dictation were lost (up to several seconds on a long text). Tests were also unstable: on an already closed channel, `select!` picked a branch at random.

## Decision

Whisper and the LLM compute in `spawn_blocking`, with their state in an `Arc`. `select!` is `biased`: the transcription is polled first.

## Consequences

- A new key press is seen at once and starts a new recording.
- A running computation cannot be interrupted; if the call is dropped its result is ignored and the model's lock acts as a queue.
- The loop's tests are now deterministic.
