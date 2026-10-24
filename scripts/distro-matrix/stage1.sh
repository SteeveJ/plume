#!/bin/bash
# Stage 1, runs inside a container (see run.sh). Installs the build prerequisites with the distribution's package manager,
# builds the release binary exactly as the README says, then builds the real-model test executables for stage 2.
# Env: ID (label), PKGS (install command), FEATURES ("" or "cuda"); with cuda also CUDA_PATH and CMAKE_CUDA_ARCHITECTURES.
set -uo pipefail
OUT=/out2/$ID; : > $OUT.s1
say() { echo "$1" | tee -a $OUT.s1; }
t0=$(date +%s); . /etc/os-release; say "id=$ID"; say "os=$PRETTY_NAME"; say "features=${FEATURES:-none}"
bash -c "$PKGS" > $OUT.pkgs.log 2>&1 || { say "packages=FAIL"; tail -n 12 $OUT.pkgs.log; exit 10; }
export PATH=/root/.cargo/bin:/usr/local/cuda/bin:$PATH
command -v cargo >/dev/null || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal > $OUT.rustup.log 2>&1 || { say "rustup=FAIL"; exit 11; }
export CARGO_TARGET_DIR=/build; cd /src
F=""; [ -n "${FEATURES:-}" ] && F="--features $FEATURES"
t1=$(date +%s)
if cargo build --release --locked $F > $OUT.build.log 2>&1; then say "build=ok ($(( $(date +%s) - t1 ))s)"; else say "build=FAIL"; grep -n '^error' -A6 $OUT.build.log | head -n 25; exit 12; fi
say "binary=$(du -h /build/release/plume | cut -f1)"
if cargo test --release --locked $F --no-run --lib --test e2e_models --test e2e_french --test e2e_llm_eval > $OUT.tests-build.log 2>&1; then say "test_build=ok"; else say "test_build=FAIL"; grep -n '^error' -A6 $OUT.tests-build.log | head -n 20; exit 13; fi
sed -nE 's/.*Executable .*\((\/build\/release\/deps\/[^)]*)\).*/\1/p' $OUT.tests-build.log > $OUT.exes
say "executables=$(wc -l < $OUT.exes)"; say "stage1_total=$(( $(date +%s) - t0 ))s"
