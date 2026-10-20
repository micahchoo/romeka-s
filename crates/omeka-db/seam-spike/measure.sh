#!/usr/bin/env bash
set -eu
# Run inside gen.py's OUT. Each build edits main.rs, because cargo fingerprints by content.
export CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=${CARGO_TARGET_DIR:?set a target dir}/seam-spike CARGO_BUILD_JOBS=8
for profile in dev release; do
  flag=""; dir=debug
  [ $profile = release ] && flag="--release" && dir=release
  cargo build $flag -q 2>/dev/null
  for v in spike-generic spike-enum; do
    times=()
    for r in 1 2 3; do
      echo "// $RANDOM$r" >> $v/src/main.rs
      s=$(date +%s.%N); cargo build $flag -q -p $v 2>/dev/null; e=$(date +%s.%N)
      times+=($(echo "$e - $s" | bc))
    done
    size=$(stat -c %s $CARGO_TARGET_DIR/$dir/$v)
    echo "$profile $v times=${times[*]} size=$size"
  done
done
