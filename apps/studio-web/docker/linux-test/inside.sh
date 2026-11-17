#!/usr/bin/env bash
# Runs inside the container of ./Dockerfile (started by ./run.mjs, or by the linux-test job of
# .github/workflows/studio-installers.yml). F-821, ADR 0801 Addendum 19.
#
#   /io/src.tar     the repository's committed files (git archive), read only
#   /io/out/        what comes back: logs, Playwright JSON reports, summary.txt
#   /cache/target   CARGO_TARGET_DIR (a named volume, kept between runs)
#   /cache/dl       npm's and Electron's download caches (a named volume)
#
# Steps, each one logged to /io/out/<step>.log:
#   cargo    rcfd, rcfd-mcp, rcfd-runner (the Studio's engine) and the f447_bench example (the bench and data tests)
#   build    npm ci, npm run build, npm run package:dir (the packaged Linux Studio, out/packaged/rcfd-studio)
#   unit     playwright --project=unit
#   e2e      playwright --project=electron under Xvfb, as the unprivileged user `studio`
#   installers  npm run package:installers (AppImage and .deb, F-820), only when named in STUDIO_STEPS
#   bench    npm run bench (the F-817 light set), only with STUDIO_BENCH=1
#
# STUDIO_STEPS (default "cargo build unit e2e") chooses steps; STUDIO_SPECS limits e2e to the named spec files.
set -uo pipefail

OUT=/io/out
mkdir -p "$OUT"
STEPS=" ${STUDIO_STEPS:-cargo build unit e2e} "
[ "${STUDIO_BENCH:-0}" = 1 ] && STEPS="$STEPS bench "
SUMMARY="$OUT/summary.txt"
: > "$SUMMARY"
note() { echo "$*" | tee -a "$SUMMARY"; }
has() { case "$STEPS" in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

export CARGO_TARGET_DIR=/cache/target
export npm_config_cache=/cache/dl/npm
export electron_config_cache=/cache/dl/electron
export CARGO_TERM_COLOR=never
mkdir -p /cache/target /cache/dl/npm /cache/dl/electron

note "image: $(. /etc/os-release && echo "$PRETTY_NAME"), $(ldd --version | head -1), $(rustc --version), node $(node --version)"
note "steps:$STEPS"
note "chromium switches for the tests: ${RCFD_TEST_ELECTRON_ARGS:-none}"

rm -rf /work && mkdir -p /work/src
tar -xf /io/src.tar -C /work/src || { note "FAIL: could not unpack /io/src.tar"; exit 2; }
APP=/work/src/apps/studio-web
cd /work/src

status=0
step() { # step <name> <command...>: runs it, logs it, records the exit code
  local name=$1; shift
  local t0=$(date +%s)
  "$@" > "$OUT/$name.log" 2>&1
  local rc=$?
  note "$name: exit $rc ($(( $(date +%s) - t0 )) s)"
  [ $rc -ne 0 ] && status=1
  return $rc
}

if has cargo; then
  step cargo bash -c 'cargo build --release --locked -p rcfd-cli -p rcfd-mcp -p rcfd-runner \
    && cargo build --release --locked -p rcfd-api --example f447_bench' || exit 1
fi
for b in rcfd rcfd-mcp rcfd-runner examples/f447_bench; do
  [ -x "$CARGO_TARGET_DIR/release/$b" ] && note "binary: $b" || note "binary missing: $b"
done

if has build; then
  step build bash -c "cd '$APP' && npm ci --no-audit --no-fund && npm run build && npm run package:dir" || exit 1
  note "packaged: $(cat "$APP/out/packaged.json" | tr -d '\n ')"
fi
if has installers; then
  # The Linux installers of F-820 (what studio-installers.yml builds on ubuntu-24.04), and their engine checked.
  step installers bash -c "cd '$APP' && npm run package:installers -- --engine='$CARGO_TARGET_DIR/release' \
    && for f in rcfd rcfd-mcp rcfd-runner; do dpkg-deb -c out/installers/*.deb | grep -E \"resources/bin/\$f\\\$\"; done \
    && dpkg-deb --info out/installers/*.deb && ls -l out/installers" \
    && mkdir -p "$OUT/installers" && cp "$APP"/out/installers/*.AppImage "$APP"/out/installers/*.deb "$OUT/installers/"
fi
chown -R studio:studio "$APP"

# As `studio`, with a display. The binaries are found through CARGO_TARGET_DIR, as on Windows (tests/support/*.ts).
as_studio() {
  runuser -u studio -- env HOME=/home/studio CARGO_TARGET_DIR=/cache/target CI=1 \
    RCFD_TEST_ELECTRON_ARGS="${RCFD_TEST_ELECTRON_ARGS:-}" \
    PLAYWRIGHT_JSON_OUTPUT_NAME="$1" bash -c "cd '$APP' && $2"
}

if has unit; then
  step unit as_studio "$OUT/unit.json" "npx playwright test --project=unit --reporter=list,json"
fi
if has e2e; then
  specs=""
  for s in ${STUDIO_SPECS:-}; do specs="$specs tests/e2e/$s"; done
  step e2e as_studio "$OUT/e2e.json" \
    "dbus-run-session -- xvfb-run -a -s '-screen 0 1920x1080x24' npx playwright test --project=electron --reporter=list,json $specs"
fi
if has bench; then
  step bench as_studio "$OUT/bench-unused.json" \
    "dbus-run-session -- xvfb-run -a -s '-screen 0 1920x1080x24' npm run bench -- --out=$OUT/bench.json"
fi

# Counts from the Playwright JSON reports (passed, failed, skipped, flaky) for summary.txt.
for r in unit e2e; do
  [ -f "$OUT/$r.json" ] || continue
  node -e '
    const j = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    const s = j.stats || {};
    console.log(`${process.argv[2]} counts: ${s.expected ?? 0} passed, ${s.unexpected ?? 0} failed, ${s.skipped ?? 0} skipped, ${s.flaky ?? 0} flaky`);
  ' "$OUT/$r.json" "$r" | tee -a "$SUMMARY"
done
exit $status
