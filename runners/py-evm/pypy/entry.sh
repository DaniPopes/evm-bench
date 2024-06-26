#!/usr/bin/env bash
set -eo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

cd "$SCRIPT_DIR"
poetry env use pypy3 >&2
poetry install >&2
poetry update >&2
poetry run python ../runner.py "$@"
