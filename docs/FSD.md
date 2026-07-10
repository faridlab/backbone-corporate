# backbone-corporate — FSD

## Entities
Currency (`iso_code` unique/@max(3), `name`, `symbol?`, `decimal_places` default 2, `is_active`) ·
CurrencyExchange (`company_id?` logical FK to `organization.Company.id` — null = global, `from_currency`,
`to_currency`, `rate` `@precision(20,10)` `@non_negative`, `effective_from`, `effective_to?` — open-ended =
current; index `(from_currency, to_currency, effective_from)`, index `(company_id)`) · Territory (`code`
unique, `name`, `parent_id?` self-FK — a tree, `is_group`) · Incoterm (`code` unique `@max(8)`, `name`,
`is_active`) · TermsAndConditions (`code` unique, `title`, `body` `@max(8000)`, `is_active`).

## Write path (`FxService`, hand-authored, user-owned — `application::service::fx_service`)
- `upsert_rate(NewRate) -> Uuid` — registers a directed, effective-dated rate. Rejects: `from == to`, a
  non-positive rate, `effective_to < effective_from`, and an overlapping window for the same
  `(from, to, company_id)` scope (checked in the same tx as the insert; the DB CHECK/EXCLUDE backstop the
  same boundary against every other writer).
- `convert(company_id, amount, from, to, on_date) -> Converted` — the load-bearing read. Same-currency is
  the identity. Otherwise: direct lookup (company scope wins over global, most recent `effective_from`
  wins among candidates — deterministic because overlap is prevented on write) → round to the quote
  currency's `decimal_places` (`MidpointAwayFromZero`). If no direct row: inverse fallback via the `to→from`
  row's reciprocal (`inverse = true`). Otherwise `NoRate`.
- Returns `Converted { amount, rate, rate_id: Option<Uuid>, rate_date, inverse: bool }` — the rate + row id
  are returned so the CALLER stamps them on its own transaction (the audit/revaluation record every
  foreign-currency document owes).

Errors: `FxError { Db, Invalid(String), NoRate { from, to, date }, OverlappingWindow { from, to } }`.

### DB-level backstop (maturity council 2026-07-12)
`corporate.currency_exchanges` carries `CHECK (rate > 0)` (migration `20260712000200`) so a zero/negative
rate is refused through every writer — the generic CRUD `create`/`upsert`/`bulk_create` included — not just
`upsert_rate`. This is the real trust boundary; `upsert_rate`'s app check is a friendly early error on top
of it.

## Seam (port — zero normal Cargo edge)
- **Read → any consumer (proven, FXSEAM-1):** a consumer calls `FxService::convert` directly (a
  `ConversionPort` in spirit — corporate has no outbound port of its own since it never writes to another
  module) and books the converted amount in its own ledger. Proven: a USD 100 supplier bill converts through
  the real FX engine and posts a **balanced** journal in the REAL `backbone-accounting` ledger
  (`backbone-accounting` is a dev-dependency only; corporate itself posts no GL).
- Corporate never imports another module's write path. It is read by, never a caller of, other modules.

## Test oracle
`fx_golden_cases` (6: FGC-1 rounds to the quote currency's decimal places, FGC-2 same-currency identity,
FGC-3 effective-dating reproduces history — old/new windows resolve to the rate in force on the doc date,
FGC-4 a company rate overrides the global rate, FGC-5 convert returns the rate + rate_id for stamping,
FGC-6 the inverse fallback reciprocates the SAME stamped row and round-trips to the minor unit),
`integrity_probes` (5: FIP-1 a missing rate is a hard signal — NoRate, never a guess, FIP-2 an overlapping
window is refused — the maturity invariant, FIP-3 a non-overlapping adjacent window is allowed — the
legitimate rate-change case, FIP-4 bad rate input (non-positive, self-pair) is rejected at the app layer,
FIP-5 a zero/negative rate inserted through a raw INSERT — the same trust boundary the CRUD endpoints sit
on — is rejected by the DB CHECK), `fx_accounting_seam` (1: FXSEAM-1 a converted USD bill posts a balanced
IDR journal in the REAL ledger) + `scripts/fx_seam_roundtrip.sh` (§5 regen byte-identity).
**12 tests.**
