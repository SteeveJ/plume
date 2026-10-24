#!/bin/bash
# Docker cannot cap GPU memory. This polls the container's GPU processes with nvidia-smi and kills the container above the limit.
# Usage: vram-watch.sh <container> <limit_mib> <out_file>   (the last line of the out file is the peak)
c=$1; limit=$2; out=$3; peak=0; : > $out
# the container may not exist yet: wait for it (up to 5 minutes)
for i in $(seq 1 600); do docker ps --format '{{.Names}}' | grep -qx "$c" && break; sleep 0.5; done
while docker ps --format '{{.Names}}' | grep -qx "$c"; do
  pids=$(docker top $c -eo pid 2>/dev/null | tail -n +2 | tr -d ' ' | paste -sd'|')
  [ -n "$pids" ] && used=$(nvidia-smi --query-compute-apps=pid,used_memory --format=csv,noheader,nounits 2>/dev/null | awk -F', ' -v re="^($pids)$" '$1 ~ re {s+=$2} END{print s+0}') || used=0
  [ "$used" -gt "$peak" ] && peak=$used
  if [ "$used" -gt "$limit" ]; then echo "VRAM LIMIT EXCEEDED: ${used} MiB > ${limit} MiB, killing $c" >> $out; docker kill $c >/dev/null 2>&1; break; fi
  sleep 0.5
done
echo "vram_peak_mib=$peak limit_mib=$limit" >> $out
