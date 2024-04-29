#!/usr/bin/env bash
set -eo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cargo run --profile runner --manifest-path "$SCRIPT_DIR/../Cargo.toml" -- --jit "$@"
