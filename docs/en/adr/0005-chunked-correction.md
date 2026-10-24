# ADR 0005 — LLM correction in chunks with a content-loss guard

**English** | [Français](../../fr/adr/0005-correction-par-morceaux.md)

- Status: accepted
- Date: 2026-10-24

## Context

With the production prompt ("Preserve the user's exact words"), Qwen3-0.6B returned only 374 words out of 572 in a single call: 35 % of the text silently disappeared into the document. It removes repeated sentences and skips passages.

## Decision

The text is cut into chunks of about 350 characters, at the end of a sentence, never inside an accented letter. Each chunk is corrected separately. Its correction is **rejected in favour of the raw text** if it is empty or loses more than 10 % of the words (for chunks of 12 words or more). The guard is off for styles that rewrite, where shortening is legitimate. The cursor context is sent with the first chunk only.

## Consequences

- On the test text, 100 % of the words are kept (4 chunks out of 8 fell back to raw text, so were not corrected).
- Correcting a long text takes about twice as long.
- An empty LLM output no longer erases the dictation (it did before this change).
- Tests: `processor_that_drops_content_falls_back_to_raw`, `empty_processor_output_never_erases_the_dictation`, `tests/pipeline.rs`.
