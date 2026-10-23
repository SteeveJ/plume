#!/usr/bin/env python3
"""
Persistent whisper server for plume.
Loads the model once, then reads tab-separated lines from stdin:
  <wav_path>\t<language>\t<initial_prompt>
Writes a JSON line to stdout for each input:
  {"text": "..."}
"""
import sys, json, os

model_name = sys.argv[1] if len(sys.argv) > 1 else "small"
device     = sys.argv[2] if len(sys.argv) > 2 else "cuda"

import whisper
model = whisper.load_model(model_name, device=device)
print(json.dumps({"ready": True}), flush=True)

for line in sys.stdin:
    parts = line.rstrip("\n").split("\t", 2)
    if not parts or not parts[0]:
        continue
    path   = parts[0]
    lang   = parts[1] if len(parts) > 1 and parts[1] else None
    prompt = parts[2] if len(parts) > 2 and parts[2] else None

    kwargs = {"fp16": device != "cpu"}
    if lang:   kwargs["language"]        = lang
    if prompt: kwargs["initial_prompt"] = prompt

    try:
        result = model.transcribe(path, **kwargs)
        # avg no_speech_prob across segments (0=speech, 1=silence/noise)
        segs = result.get("segments", [])
        no_speech = sum(s.get("no_speech_prob", 0) for s in segs) / len(segs) if segs else 0.0
        print(json.dumps({"text": result["text"], "no_speech_prob": no_speech}), flush=True)
    except Exception as e:
        print(json.dumps({"error": str(e)}), flush=True)
