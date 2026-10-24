# Configuration

Plume reads `~/.config/plume.conf` (created on first launch, mode `600`). The settings window (tray → *Settings*) edits the same file and **keeps every key it does not show**. Changes apply after `plume --restart` or tray → *Reload config*.

Unknown keys and invalid values are reported by a notification at startup and in the log (for example `PROCESSOR=olama`).

## Keys

| Key | Default | Meaning |
|-----|---------|---------|
| `HOTKEY` | `KEY_RIGHTCTRL` | Recording key: `KEY_RIGHTCTRL`, `KEY_LEFTCTRL`, `KEY_RIGHTALT`, `KEY_LEFTALT`, `KEY_RIGHTMETA`, `KEY_CAPSLOCK`, `KEY_SCROLLLOCK`, `KEY_PAUSE`, `KEY_INSERT`, `KEY_F1` … `KEY_F12` |
| `MODE` | `hold` | `hold` = record while pressed; `toggle` = press to start, press to stop |
| `STT_LANG` | empty | Spoken language (`fr`, `en`, …). Empty = automatic detection |
| `STT_MODEL` | `small` | `tiny`, `base`, `small`, `medium`, `large-v2`, `turbo`, or a quantized variant such as `small-q8_0`, `small-q5_1`, `large-v3-turbo-q5_0` |
| `STT_PROMPT` | empty | Words Whisper should favour (names, jargon) |
| `PROCESSOR` | `local` | `local` (Qwen3 0.6B), `openai`, `anthropic`, `ollama`, `none` |
| `PROCESSOR_STYLE` | `clean` | `clean` fixes errors only. Others rewrite and need an online model: `casual`, `excited`, `formal`, `professional`, `diplomatic` (alias `soften`), `assertive`, `humor` |
| `INSTRUCT_HOTKEY` | empty (off) | Second key for **instruction mode**: select text in any app, hold this key and say what to do with it ("make it more formal", "translate to English"). Needs `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`; the selected text is sent to that service. Same values as `HOTKEY`, must differ from it |
| `OUTPUT` | `inject` | `inject` types into the focused window; `scratchpad` appends to `scratch.txt` and the clipboard; `both` |
| `APP_LANG` | `en` | Language of notifications and the settings window (`en`, `fr`) |
| `OPENAI_API_KEY`, `OPENAI_PROCESSOR_MODEL` | — / `gpt-4o-mini` | Used by `PROCESSOR=openai`, online styles, and as speech fallback |
| `ANTHROPIC_API_KEY`, `ANTHROPIC_MODEL` | — / `claude-haiku-4-5` | Used by `PROCESSOR=anthropic` |
| `OLLAMA_MODEL` | `qwen3:0.6b` | Used by `PROCESSOR=ollama` (server on `localhost:11434`) |
| `MODEL_PATH`, `MODEL_URL`, `MODEL_SHA256`, `MODEL_REPO`, `MODEL_PATTERN` | `~/.local/share/plume/model.gguf`, —, —, `unsloth/Qwen3-0.6B-GGUF`, `Q4_K_M` | Where the local LLM file is and how it is downloaded |

The environment variables `OPENAI_API_KEY` and `ANTHROPIC_API_KEY` override the file.

## Choosing the speech model

Measured on 5 minutes of audio, process memory at peak. On a GPU the weights live in VRAM and the RAM figure drops to a few hundred MB.

| `STT_MODEL` | CPU memory | Notes |
|-------------|-----------|-------|
| `small` | ~890 MB | Good default |
| `small-q8_0` | ~680 MB | Almost the same quality |
| `small-q5_1` | ~610 MB | Lightest `small` |
| `turbo` | ~2.1 GB | Best quality; use a GPU |
| `large-v3-turbo-q8_0` | ~1.35 GB | |
| `large-v3-turbo-q5_0` | ~1.06 GB | |

Plume refuses a model that cannot fit in free RAM and tells you which one to pick instead. Compare quality on your own voice before keeping a quantized model.

## Files in `~/.local/share/plume/`

| File | Content |
|------|---------|
| `ggml-*.bin` | Whisper model(s), downloaded on demand |
| `ggml-silero-v5.1.2.bin` | Voice detector (885 KB), downloaded with the first Whisper model |
| `model.gguf` | Local LLM |
| `vocab.txt` | Words fed to Whisper. Lines starting with `#` are comments. Names and acronyms are appended automatically |
| `snippets.txt` | One rule per line: `spoken phrase => text`. `\n` is a line break. Matching ignores case (accented capitals included), whole words only, longest phrase first. Applied after the LLM. Read at startup |
| `history.db` | Every dictation (raw and final text), last 10 000 kept, mode `600` |
| `scratch.txt` | Output of `OUTPUT=scratchpad`, mode `600` |

The data folder is mode `700`.
