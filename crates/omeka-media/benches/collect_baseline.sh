#!/usr/bin/env bash
# The benchmark lever: run this crate's criterion benches, then write
# `benches/baseline.json` from the estimates criterion just produced.
#
#   ./benches/collect_baseline.sh              # every group in the crate
#   ./benches/collect_baseline.sh value_rules  # criterion filters pass through
#
# The script derives the crate from its own path, so the same file works in
# every benched crate. Rerunning overwrites `baseline.json`; commit the change
# only when the shift is intended, and say so in the commit message.
#
# Requires python3 and a release bench profile (criterion always uses one).
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
crate=$(basename "$(dirname "$here")")
target=${CARGO_TARGET_DIR:-$(cd "$here/../../.." && pwd)/target}
criterion="$target/criterion"

# criterion writes every crate's results into one directory. Drop the previous
# `new` trees so the collected file holds this crate's benches only.
if [ -d "$criterion" ]; then
    find "$criterion" -type d -name new -prune -exec rm -rf {} +
fi

cargo bench -p "$crate" -- "$@"

python3 - "$criterion" "$here/baseline.json" "$crate" <<'PYTHON'
import json
import os
import subprocess
import sys

criterion_dir, output, crate = sys.argv[1], sys.argv[2], sys.argv[3]


def run(*command):
    try:
        return subprocess.run(
            command, capture_output=True, text=True, check=False
        ).stdout.strip()
    except OSError:
        return ""


def bench_entry(directory):
    with open(os.path.join(directory, "estimates.json")) as handle:
        estimates = json.load(handle)
    with open(os.path.join(directory, "sample.json")) as handle:
        sample = json.load(handle)
    median = estimates["median"]
    return {
        "median_ns": median["point_estimate"],
        "median_lower_ns": median["confidence_interval"]["lower_bound"],
        "median_upper_ns": median["confidence_interval"]["upper_bound"],
        "mean_ns": estimates["mean"]["point_estimate"],
        "std_dev_ns": estimates["std_dev"]["point_estimate"],
        "samples": len(sample["iters"]),
        "iterations": sum(sample["iters"]),
    }


benches = {}
for root, _, files in os.walk(criterion_dir):
    if os.path.basename(root) != "new" or "estimates.json" not in files:
        continue
    name = os.path.relpath(os.path.dirname(root), criterion_dir)
    benches[name] = bench_entry(root)

load = os.getloadavg()
document = {
    "crate": crate,
    "profile": "bench (criterion's optimized profile)",
    "commit": run("git", "rev-parse", "HEAD"),
    "rustc": run("rustc", "--version"),
    "cpus": os.cpu_count(),
    "load_average": [round(value, 2) for value in load],
    "criterion_dir": criterion_dir,
    "note": (
        "samples is criterion's sample count; iterations is the total number of "
        "timed units it executed (the sum of its per-sample iteration counts). "
        "For iter_batched benches one unit is one batch and its setup runs "
        "outside the timed region"
    ),
    "benches": benches,
}
with open(output, "w") as handle:
    json.dump(document, handle, indent=2, sort_keys=True)
    handle.write("\n")
print(f"wrote {len(benches)} benches to {output}")
PYTHON
