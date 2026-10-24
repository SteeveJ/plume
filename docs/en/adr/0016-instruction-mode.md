# ADR 0016 — Instruction mode: rewrite the selection with an online model

**English** | [Français](../../fr/adr/0016-mode-instruction.md)

- Status: accepted
- Date: 2026-10-24

## Context

We want to select text in any application, say "make it more formal" or "translate to English", and get the rewritten text in its place. The small local model (Qwen3-0.6B) rewrites too poorly: it changes or drops words (ADR 0004 and 0005). An online model is needed. Reading the selected text could be done three ways: simulating Ctrl+C (dangerous: in a terminal it is SIGINT and interrupts the running program), reading the selection through AT-SPI accessibility (not verified on this machine), or reading the desktop's "primary" selection (the middle-click one). Tried on the development machine (COSMIC, Wayland): the primary selection can be read with `arboard`, typing nothing.

## Decision

- A second key `INSTRUCT_HOTKEY`, **off by default**, always hold-to-talk and different from `HOTKEY` (otherwise it is ignored with a warning).
- It requires `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` (OpenAI first). Without a key Plume says so and the mode stays off.
- Flow: on press, read the primary selection (refused if empty or over 20,000 characters), the notification shows the start of the text read; record; same listening chain as dictation (volume check, voice detector, hallucination filter); the model gets `Instruction: …` and `Text: …` with the `INSTRUCT_PROMPT` brief; the result is pasted over the selection with the same injector as dictation.
- Empty answer or model error: nothing is pasted, the user's text stays intact, a notification explains why. History keeps `[instruction] …` and the result.
- A command mode ("delete the last paragraph") is **rejected**: a misheard order could delete text, and it needs a list of actions to design.

## Consequences

- **Privacy:** the selected text and the instruction are sent to the chosen provider. This is stated in the default configuration and in the docs.
- The primary selection keeps the last selected text even if the application has deselected it since: the preview in the notification and Ctrl+Z are the safety net.
- No guard against word loss (ADR 0005): a rewrite may legitimately shorten or translate.
- Tested with fake components (microphone, selection, model) and the real SQLite history; **never tested against the real API**, nor in a real application.
- A second key needs a free key: avoid Alt Gr (characters) and keys the desktop already uses.
