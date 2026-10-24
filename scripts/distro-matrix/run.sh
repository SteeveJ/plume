#!/bin/bash
# Build and use Plume on several Linux distributions at once, in containers (no VirtualBox needed).
#
#   Stage 1 (build): each container installs the build prerequisites and builds Plume as the README says.
#   Stage 2 (usage): a clean container with only the run-time libraries, limited to 2 GB of RAM, runs the real models on
#                    French speech and noise, then starts the real daemon. The GPU case also caps VRAM at 4 GiB.
#
# Needs: docker, about 30 GB of free disk, and the model files below. For the GPU case also an NVIDIA GPU and its driver
# (the driver files are mounted into the container by hand, so the nvidia-container-toolkit is not required).
#
#   MODELS_DIR=~/models ./scripts/distro-matrix/run.sh            # results in /tmp/plume-matrix/summary.txt
#
# MODELS_DIR must contain: ggml-small.bin, ggml-large-v3-turbo.bin, ggml-silero-v5.1.2.bin, model.gguf (see docs/en/configuration.md).
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$HERE/../.." && pwd)
MODELS_DIR=${MODELS_DIR:?set MODELS_DIR to the folder that holds the model files}
WORK=${WORK:-/tmp/plume-matrix}; OUT=$WORK/out; SUMMARY=$WORK/summary.txt; mkdir -p "$OUT"; : > "$SUMMARY"
VRAM_LIMIT_MIB=${VRAM_LIMIT_MIB:-4096}; RAM_LIMIT=${RAM_LIMIT:-2g}

APT='apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends build-essential cmake pkg-config curl ca-certificates git binutils bc ripgrep libclang-dev libgtk-4-dev libasound2-dev libdbus-1-dev libsqlite3-dev'
DNF='dnf install -y gcc gcc-c++ make cmake pkgconf-pkg-config curl ca-certificates git binutils bc ripgrep clang-devel gtk4-devel alsa-lib-devel dbus-devel sqlite-devel'
PAC='pacman -Syu --noconfirm base-devel cmake clang gtk4 alsa-lib dbus sqlite git curl ripgrep bc'
RT_APT='apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends libgtk-4-1 libasound2 libdbus-1-3 libsqlite3-0 ca-certificates'
RT_APT24='apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends libgtk-4-1 libasound2t64 libdbus-1-3 libsqlite3-0 ca-certificates'
RT_DNF='dnf install -y gtk4 alsa-lib dbus-libs sqlite-libs ca-certificates'
RT_PAC='pacman -Syu --noconfirm gtk4 alsa-lib dbus sqlite'
COMMON="-v $OUT:/out2 -v $REPO:/src:ro -v plume-cargo:/root/.cargo -v plume-rustup:/root/.rustup"
ENVM="-e PLUME_TEST_GGML=/models/ggml.bin -e PLUME_TEST_GGUF=/models/model.gguf -e PLUME_TEST_VAD=/models/vad.bin"

cpu_case() { # id image build-packages run-packages
  local R=$OUT/$1.summary; echo "=== $1 ($2)" > "$R"
  docker run --rm --name "plume-s1-$1" --cpus 2 --memory 3g --memory-swap 3g -e ID="$1" -e PKGS="$3" $COMMON \
    -v "$HERE/stage1.sh:/s.sh:ro" -v "plume-build-$1:/build" "$2" bash /s.sh > "$OUT/$1.s1.console" 2>&1
  local rc=$?; cat "$OUT/$1.s1" >> "$R" 2>/dev/null
  [ $rc -ne 0 ] && { echo "stage1 exit=$rc (no usage test)" >> "$R"; return; }
  docker run --rm --name "plume-s2-$1" --memory "$RAM_LIMIT" --memory-swap "$RAM_LIMIT" -e ID="$1" -e RUNPKGS="$4" $ENVM \
    -e DEFAULT_MODEL_FILE=ggml-small.bin -e GUARD_TEST=1 -e QUICK=${QUICK:-0} $COMMON \
    -v "$HERE/stage2.sh:/s.sh:ro" -v "plume-build-$1:/build:ro" \
    -v "$MODELS_DIR/ggml-large-v3-turbo.bin:/models/turbo.bin:ro" -v "$MODELS_DIR/ggml-small.bin:/models/ggml.bin:ro" \
    -v "$MODELS_DIR/ggml-silero-v5.1.2.bin:/models/vad.bin:ro" -v "$MODELS_DIR/model.gguf:/models/model.gguf:ro" \
    "$2" bash /s.sh > "$OUT/$1.s2.console" 2>&1
  echo "stage2 exit=$?" >> "$R"; cat "$OUT/$1.s2" >> "$R" 2>/dev/null
}

gpu_case() {
  local ID=ubuntu2204-cuda R=$OUT/ubuntu2204-cuda.summary; echo "=== $ID (nvidia/cuda 12.8.1 devel, GPU)" > "$R"
  docker run --rm --name "plume-s1-$ID" --cpus 4 --memory 6g --memory-swap 6g -e ID=$ID -e PKGS="$APT" -e FEATURES=cuda \
    -e CUDA_PATH=/usr/local/cuda -e CMAKE_CUDA_ARCHITECTURES=120 $COMMON -v "$HERE/stage1.sh:/s.sh:ro" -v "plume-build-$ID:/build" \
    nvidia/cuda:12.8.1-devel-ubuntu22.04 bash /s.sh > "$OUT/$ID.s1.console" 2>&1
  local rc=$?; cat "$OUT/$ID.s1" >> "$R" 2>/dev/null
  [ $rc -ne 0 ] && { echo "stage1 exit=$rc" >> "$R"; return; }
  "$HERE/vram-watch.sh" "plume-s2-$ID" "$VRAM_LIMIT_MIB" "$OUT/$ID.vram" &
  # GPU devices and the driver libraries of the host are passed by hand. 120 = RTX 50 series: change CMAKE_CUDA_ARCHITECTURES above for another card.
  docker run --rm --name "plume-s2-$ID" --memory "$RAM_LIMIT" --memory-swap "$RAM_LIMIT" -e ID=$ID -e RUNPKGS="$RT_APT" \
    -e LD_LIBRARY_PATH=/usr/local/nvidia/lib $ENVM -e DEFAULT_MODEL_FILE=ggml-large-v3-turbo.bin -e DEFAULT_STT_MODEL=turbo -e GUARD_TEST=0 \
    --device /dev/nvidia0 --device /dev/nvidiactl --device /dev/nvidia-uvm --device /dev/nvidia-uvm-tools --device /dev/nvidia-modeset \
    -v /usr/lib/libcuda.so.1:/usr/local/nvidia/lib/libcuda.so.1:ro -v /usr/lib/libnvidia-ptxjitcompiler.so.1:/usr/local/nvidia/lib/libnvidia-ptxjitcompiler.so.1:ro \
    -v /usr/lib/libnvidia-nvvm.so.4:/usr/local/nvidia/lib/libnvidia-nvvm.so.4:ro -v /usr/lib/libnvidia-cfg.so.1:/usr/local/nvidia/lib/libnvidia-cfg.so.1:ro \
    $COMMON -v "$HERE/stage2.sh:/s.sh:ro" -v "plume-build-$ID:/build:ro" \
    -v "$MODELS_DIR/ggml-large-v3-turbo.bin:/models/ggml.bin:ro" -v "$MODELS_DIR/ggml-silero-v5.1.2.bin:/models/vad.bin:ro" -v "$MODELS_DIR/model.gguf:/models/model.gguf:ro" \
    ubuntu:22.04 bash /s.sh > "$OUT/$ID.s2.console" 2>&1
  echo "stage2 exit=$?" >> "$R"; cat "$OUT/$ID.s2" >> "$R" 2>/dev/null; sleep 2; echo "vram: $(tail -n 2 "$OUT/$ID.vram" | tr '\n' ' ')" >> "$R"
}

# Three cases at a time (MAX_PARALLEL): six CPU evaluations of the LLM at once share the processor and take 20+ minutes each.
# Builds are capped (2 cores and 3 GB each; CUDA 4 cores and 6 GB) so the desktop stays usable.
MAX_PARALLEL=${MAX_PARALLEL:-3}
throttle() { while [ "$(jobs -rp | wc -l)" -ge "$MAX_PARALLEL" ]; do wait -n; done; }
[ "${SKIP_GPU:-0}" = 1 ] || { throttle; gpu_case & }
throttle; cpu_case ubuntu2204 ubuntu:22.04 "$APT" "$RT_APT" &
throttle; cpu_case ubuntu2404 ubuntu:24.04 "$APT" "$RT_APT24" &
throttle; cpu_case debian12   debian:12 "$APT" "$RT_APT" &
throttle; cpu_case fedora38   fedora:38 "$DNF" "$RT_DNF" &
throttle; cpu_case fedora42   fedora:42 "$DNF" "$RT_DNF" &
throttle; cpu_case archlinux  archlinux:latest "$PAC" "$RT_PAC" &
wait
cat "$OUT"/*.summary > "$SUMMARY"; echo "done: $SUMMARY"
