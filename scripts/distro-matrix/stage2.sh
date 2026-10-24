#!/bin/bash
# Stage 2, runs inside a clean, resource-limited container that has ONLY the run-time libraries (a user's machine, not a build machine).
# A: the real-model usage tests. B: the real daemon with default settings (must load both models, then stop cleanly at uinput).
# C: a model too big for the limit (must be refused with a message, not killed by the kernel).
# Env: ID, RUNPKGS, DEFAULT_MODEL_FILE, DEFAULT_STT_MODEL, GUARD_TEST (1 = run C), QUICK (1 = skip the two slow CPU evaluations).
set -uo pipefail
OUT=/out2/$ID; : > $OUT.s2
say() { echo "$1" | tee -a $OUT.s2; }
t0=$(date +%s); . /etc/os-release; say "os=$PRETTY_NAME"
say "memory_limit_mb=$(( $(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo 0) / 1048576 ))"
say "meminfo_available_mb=$(awk '/MemAvailable/{print int($2/1024)}' /proc/meminfo) (what /proc/meminfo claims)"
bash -c "$RUNPKGS" > $OUT.runpkgs.log 2>&1 || { say "runtime_packages=FAIL"; tail -n 10 $OUT.runpkgs.log; exit 20; }
say "runtime_packages=ok"
miss=$(ldd /build/release/plume 2>&1 | grep -c 'not found'); say "ldd_missing=$miss"
[ "$miss" != "0" ] && ldd /build/release/plume | grep 'not found'
cd /src; fail=0

# --- A. the real-model usage tests (French speech, noise, voice detector, correction by the LLM)
for exe in $(cat $OUT.exes); do
  name=$(basename $exe | sed -E 's/-[0-9a-f]+$//')
  # QUICK=1 skips the two slow CPU evaluations (already validated); the daemon scenarios below always run
  if [ "${QUICK:-0}" = 1 ]; then case $name in e2e_french|e2e_llm_eval) continue ;; esac; fi
  case $name in
    plume) args="--ignored whisper_memory --nocapture" ;;
    *) args="--ignored --nocapture --test-threads=1" ;;
  esac
  t=$(date +%s); $exe $args > $OUT.$name.log 2>&1; rc=$?
  res=$(grep -h 'test result' $OUT.$name.log | tail -n1 | sed -E 's/^test result: ([a-zA-Z]+)\. ([0-9]+) passed; ([0-9]+) failed.*/\1 passed=\2 failed=\3/')
  if [ $rc -eq 0 ]; then verdict=ok; else verdict="FAIL_rc_$rc"; fail=1; fi
  say "usage_$name=$verdict [$res] ($(( $(date +%s) - t ))s)"
  [ $rc -ne 0 ] && grep -E 'panicked|FAILED' $OUT.$name.log | head -n 3
done

# --- B. the real daemon, headless, default settings: it must load both models inside the limit,
#        then stop with a clear message at the first thing a container lacks (uinput)
H=/tmp/home-a; mkdir -p $H/.local/share/plume
ln -sf /models/ggml.bin $H/.local/share/plume/$DEFAULT_MODEL_FILE
ln -sf /models/model.gguf $H/.local/share/plume/model.gguf
ln -sf /models/vad.bin $H/.local/share/plume/ggml-silero-v5.1.2.bin
[ -n "${DEFAULT_STT_MODEL:-}" ] && { mkdir -p $H/.config; echo "STT_MODEL=$DEFAULT_STT_MODEL" > $H/.config/plume.conf; }
t=$(date +%s); HOME=$H timeout 240 /build/release/plume > $OUT.daemon.log 2>&1; rc=$?
loaded=$(grep -c 'using local whisper.cpp' $OUT.daemon.log); llm=$(grep -c 'processor: local llama' $OUT.daemon.log)
clean=$(grep -ci 'uinput' $OUT.daemon.log)
if [ "$rc" = "137" ] || grep -qi 'killed' $OUT.daemon.log; then verdict="KILLED(oom)"; fail=1
elif [ "$loaded" -ge 1 ] && [ "$llm" -ge 1 ] && [ "$clean" -ge 1 ]; then verdict="ok (models loaded, clean stop at uinput)"
else verdict="UNEXPECTED rc=$rc whisper=$loaded llm=$llm uinput_msg=$clean"; fail=1; fi
say "daemon_default=$verdict ($(( $(date +%s) - t ))s)"
[[ "$verdict" == UNEXPECTED* ]] && tail -n 8 $OUT.daemon.log

# --- C. a model too big for the limit: refused with a message, never killed by the kernel
if [ "${GUARD_TEST:-0}" = "1" ]; then
  H=/tmp/home-b; mkdir -p $H/.local/share/plume $H/.config
  ln -sf /models/turbo.bin $H/.local/share/plume/ggml-large-v3-turbo.bin
  ln -sf /models/model.gguf $H/.local/share/plume/model.gguf
  echo "STT_MODEL=turbo" > $H/.config/plume.conf
  HOME=$H timeout 240 /build/release/plume > $OUT.guard.log 2>&1; rc=$?
  if [ "$rc" = "137" ] || grep -qi '^killed' $OUT.guard.log; then verdict="KILLED(oom) — the guard did not protect"; fail=1
  elif grep -q 'needs about' $OUT.guard.log; then verdict="ok (refused: $(grep -o 'needs about [0-9]* MB of free RAM but only [0-9]* MB' $OUT.guard.log | head -n1))"
  else verdict="UNEXPECTED rc=$rc"; fail=1; tail -n 6 $OUT.guard.log; fi
  say "guard_turbo_cpu=$verdict"
fi
say "memory_peak_mb=$(( $(cat /sys/fs/cgroup/memory.peak 2>/dev/null || echo 0) / 1048576 )) (includes reclaimable file cache)"
say "oom_kills=$(grep -h oom_kill /sys/fs/cgroup/memory.events 2>/dev/null | awk '{print $2}')"
say "stage2_total=$(( $(date +%s) - t0 ))s"; exit $fail
