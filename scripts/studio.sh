#!/bin/sh
# Build the Studio, the run service, the solver and the MCP server in release, then start the Studio (ADR 0141).
#
#   scripts/studio.sh [--no-build] [--dry-run] [PROJECT_DIR]
#
# --no-build skips the build. --dry-run prints the two commands, one argument per line ("build> ", "start> "), and
# runs nothing. CARGO names the cargo program (default: cargo); CARGO_TARGET_DIR is honoured. The project folder is
# passed to the Studio as given (relative to the current folder, which the Studio keeps).
usage='usage: studio.sh [--no-build] [--dry-run] [PROJECT_DIR]'
build=1
dry=0
project=
have_project=0
for a in "$@"; do
    case "$a" in
        --no-build) build=0 ;;
        --dry-run) dry=1 ;;
        -h | --help)
            echo "$usage"
            exit 0
            ;;
        -?*)
            printf "studio.sh: unknown option '%s'\n%s\n" "$a" "$usage" >&2
            exit 2
            ;;
        *)
            if [ "$have_project" = 1 ]; then
                printf "studio.sh: expected at most one project folder, got '%s' and '%s'\n%s\n" "$project" "$a" "$usage" >&2
                exit 2
            fi
            project=$a
            have_project=1
            ;;
    esac
done
root=$(cd "$(dirname "$0")/.." && pwd)
cargo=${CARGO:-cargo}
target=${CARGO_TARGET_DIR:-$root/target}
case "$(uname -s 2>/dev/null)" in
    MINGW* | MSYS* | CYGWIN*) suffix=.exe ;;
    *) suffix= ;;
esac
exe=$target/release/rcfd-studio$suffix
if [ "$dry" = 1 ]; then
    if [ "$build" = 1 ]; then
        for x in "$cargo" build --manifest-path "$root/Cargo.toml" --release \
            -p rcfd-studio -p rcfd-runner -p rcfd-cli -p rcfd-mcp; do
            printf 'build> %s\n' "$x"
        done
    fi
    printf 'start> %s\n' "$exe"
    if [ "$have_project" = 1 ]; then printf 'start> %s\n' "$project"; fi
    exit 0
fi
if [ "$build" = 1 ]; then
    "$cargo" build --manifest-path "$root/Cargo.toml" --release -p rcfd-studio -p rcfd-runner -p rcfd-cli -p rcfd-mcp
    code=$?
    if [ "$code" != 0 ]; then
        echo "studio.sh: the build failed (exit $code); the Studio is not started" >&2
        exit "$code"
    fi
fi
if [ ! -f "$exe" ]; then
    echo "studio.sh: $exe not found; build it first (leave out --no-build)" >&2
    exit 1
fi
if [ "$have_project" = 1 ]; then exec "$exe" "$project"; else exec "$exe"; fi
