# ADR 0013 — Four test levels, real models and real bugs

**English** | [Français](../../fr/adr/0013-strategie-de-tests.md)

- Status: accepted
- Date: 2026-10-24

## Context

The most serious defects of this period were invisible at compile time: a clash between two ggml copies that crashes at run time, text invented by Whisper on noise, an LLM that copies a suffix or deletes passages. Tests made only of doubles would not have found them.

## Decision

Four levels, from fastest to most realistic:
1. **Unit** (`cargo test --lib`): pure computations, no model or hardware.
2. **Integration with fake hardware** (`config_load`, `pipeline`, `hallucinations`, `french`): real adapters (real config file in a temporary `HOME`, SQLite, scratchpad), fake microphone, keyboard and Whisper.
3. **End to end with real models** (`e2e_models`, `e2e_french`, ignored by default): Whisper and Qwen in one process, real noise, French synthetic voice (Piper, `tests/fixtures/fr/`). Tests that load the LLM run one after the other: llama.cpp initializes only once per process.
4. **Real memory**: RAM really drops on unload.

Rules: one test = one bug actually met; check that it **fails** when the bug is put back; comprehension thresholds are calibrated on `turbo` and `small`.

## Consequences

- Real-model tests are run by hand (large files) but have already caught bugs nothing else saw (`[BLANK_AUDIO]`, copied `/nothink`, empty LLM on a long text).
- The synthetic voice is cleaner than a real microphone: real scores will be a little worse.
- French is tested for comprehension: meaning kept, accents, numbers written as digits, automatic language detection.
