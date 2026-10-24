# Troubleshooting

Plume is started without a terminal, so failures are shown as desktop notifications. For the full log run `plume` from a terminal (`RUST_LOG=debug plume`).

| Symptom | Cause and fix |
|---------|---------------|
| Nothing happens when I press the key | uinput or input permission. Install `assets/99-plume.rules`, join the `input` group, log out and back in. Plume keeps looking for a keyboard every 3 s, so a keyboard plugged in later works |
| "Plume could not start: … uinput" | Same as above |
| "Microphone unavailable: …" | No microphone, or it is busy. Plume keeps running; fix the microphone and press the key again |
| "No sound — is the microphone muted?" | The audio was almost silent: muted or disconnected microphone |
| "Nothing heard — speak closer to the mic" | Whisper heard only noise, or an invented phrase such as "Thank you." was filtered |
| A very quiet "merci" is ignored | Only happens if the voice detector file `ggml-silero-v5.1.2.bin` is missing (it is downloaded on first launch). Without it, voices below the noise threshold are treated as noise for the few known invented phrases |
| Download seems stuck on first launch | The speech model is large (`small` 470 MB, `turbo` 1.6 GB). A notification announces it; it stops with an error after 60 s without data |
| "needs about … MB of free RAM" | The chosen `STT_MODEL` does not fit. Pick a smaller or quantized one (`small-q8_0`, `large-v3-turbo-q5_0`) or free memory |
| Transcription takes seconds | You are on CPU. Build with `--features cuda` (NVIDIA). `turbo` on CPU needs about 4 s for 11 s of audio |
| The text is not corrected | The 0.6B model sometimes returns the text unchanged, and on long texts it can fail to answer; Plume then keeps your raw words. A larger local LLM (Qwen3-1.7B) was tested and is not better; try an online `PROCESSOR` instead |
| "Select the text to change first" (instruction mode) | Nothing is selected, or the application does not publish its selection. Select the text with the mouse or keyboard, then hold the instruction key. Terminals and some apps may not set it |
| "Rewrite failed: …" (instruction mode) | The online model refused (wrong key, no credit, no network). Your text is untouched |
| The instruction replaced the wrong text | The desktop keeps the last selection even after an app deselects it. The notification shows the start of the text read; press Ctrl+Z to undo |
| Settings do not apply | Use *Save & Restart*, tray → *Reload config*, or `plume --restart` |
| Not enough disk space | Plume checks free space before a download and says how much is needed |

## Useful commands

```bash
plume --restart          # restart the daemon
ls -l ~/.config/plume.conf ~/.local/share/plume/   # files should be -rw-------
nvidia-smi               # GPU memory used by plume
```
