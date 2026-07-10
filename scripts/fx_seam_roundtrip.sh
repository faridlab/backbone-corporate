#!/usr/bin/env bash
# §5 round-trip: the hand-authored FX engine (effective-dated convert + no-overlap guard + inverse fallback)
# and its oracle survive a codegen --force regen byte-identical.
set -euo pipefail
cd "$(dirname "$0")/.."
export DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5433/backbone_corporate}"
FILES=(src/application/service/fx_service.rs tests/fx_golden_cases.rs tests/integrity_probes.rs tests/fx_accounting_seam.rs)
before=$(shasum "${FILES[@]}")
echo "== regenerating (--force) =="
metaphor schema schema generate --force >/dev/null
after=$(shasum "${FILES[@]}")
if [[ "$before" != "$after" ]]; then echo "FAIL: user-owned files changed across regen"; diff <(echo "$before") <(echo "$after"); exit 1; fi
echo "OK: FX engine + oracle byte-identical across regen"
echo "== re-running the oracle + seam =="
cargo test --test fx_golden_cases --test integrity_probes --test fx_accounting_seam 2>&1 | grep -E "test result"
echo "OK: §5 round-trip holds"
