#!/usr/bin/env bash
# Reproduce a generated corpus with checksum-pinned CC0 source data.
set -euo pipefail
cd "$(dirname "$0")/../.."
root="$PWD"
work="${CBVAULT_CORPUS_WORK:-$root/fixtures-local/benchmark-work}"
mkdir -p "$work"
case "${1:-}" in
    smoke) count=25000; source=2015-11; seed=20261125; mode=realistic; profile=recipe-profile.json ;;
    standard) count=1000000; source=2015-11; seed=20261125; mode=realistic; profile=recipe-profile.json ;;
    scale) count=3410497; source=2015-11; seed=20261125; mode=realistic; profile=recipe-profile.json ;;
    rich) count=100000; source=2013-01; seed=20261126; mode=rich; profile=recipe-profile.json ;;
    extreme) count=10000; source=2013-01; seed=20261127; mode=rich; profile=extreme-profile.json ;;
    *) echo 'Usage: build-corpus.sh smoke|standard|scale|rich|extreme' >&2; exit 2 ;;
esac
case "$source" in
    2013-01) hash=aa40b3671fa3cf1072eb182892cd90b0e1e003a4a5943492f64b77e7f3fd1635 ;;
    2015-11) hash=ae64086175cb8b041743572c3694fd9fdd7410a54251b3b112f865ca53279141 ;;
esac
in="$work/lichess-$source.pgn.zst"
if [[ ! -f "$in" ]]; then
    curl -fL --retry 2 "https://database.lichess.org/standard/lichess_db_standard_rated_$source.pgn.zst" -o "$in.part"
    printf '%s  %s\n' "$hash" "$in.part" | sha256sum -c -
    mv "$in.part" "$in"
fi
printf '%s  %s\n' "$hash" "$in" | sha256sum -c -
export CARGO_TARGET_DIR="$work/cargo-target"
cargo build --locked --release --manifest-path scripts/bench-corpus/generator/Cargo.toml
g="$CARGO_TARGET_DIR/release/cbvault-benchmark-generator"
v="$CARGO_TARGET_DIR/release/validate"
out="$work/$1-$count.pgn.zst"
[[ ! -e "$out" ]] || { echo "Refusing to overwrite $out" >&2; exit 1; }
trap 'rm -f "$out.part"' EXIT
set +e
zstd -dc "$in" | "$g" "scripts/bench-corpus/$profile" "$count" "$seed" "$mode" | zstd -q -3 -T2 -o "$out.part"
status=("${PIPESTATUS[@]}")
set -e
# The prefix consumer intentionally closes upstream early. Its decompressor
# may therefore report SIGPIPE; the pinned compressed file was verified above.
test "${status[1]}" -eq 0 && test "${status[2]}" -eq 0 || exit 1
zstd -t "$out.part"
# Compare exact headers and original mainline move words.
set +e
zstd -dc "$in" | "$v" "$count" > "$out.source.json"
status=("${PIPESTATUS[@]}")
set -e
test "${status[1]}" -eq 0 || exit 1
zstd -dc "$out.part" | "$v" "$count" --exact > "$out.validation.json"
python3 - "$out.source.json" "$out.validation.json" <<'PY'
import json,sys
a,b=(json.load(open(p)) for p in sys.argv[1:])
assert a["games"]==b["games"]
assert a["mainline_plies"]==b["mainline_plies"]
assert a["mainline_and_headers_sha256"]==b["mainline_and_headers_sha256"]
print(b)
PY
mv "$out.part" "$out"
sha256sum "$out"
trap - EXIT
