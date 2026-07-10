# backbone-corporate — Extension Guide

## Public surface (stable)
- **FX engine** (`application::service::fx_service`): `FxService::new(pool)`, `convert(company_id, amount,
  from, to, on_date) -> Result<Converted, FxError>`, `upsert_rate(NewRate) -> Result<Uuid, FxError>`.
  `Converted { amount, rate, rate_id: Option<Uuid>, rate_date, inverse: bool }` — a consumer MUST stamp
  `rate` + `rate_id` on its own transaction row (the audit/revaluation record every foreign-currency
  document owes).
- **Reference masters** (generic CRUD, `presentation::http`): Currency, Territory, Incoterm,
  TermsAndConditions — the standard 12 endpoints each (list/create/get/update/patch/soft_delete/restore/
  empty_trash/bulk_create/upsert/find_by_id/list_deleted). `CurrencyExchange` also has the 12 generic
  endpoints, but writing a rate through them bypasses the overlap/positivity checks that only `upsert_rate`
  performs at the app layer (the DB `CHECK (rate > 0)` and the EXCLUDE constraint still backstop them).

## How a consuming service uses corporate
Hold a foreign amount + the transaction date, call
`fx.convert(company_id, amount, from_iso, to_iso, on_date)`, and book the returned `amount` in your own
ledger/document — **stamp `rate` and `rate_id` on your row** so a later reversal can find the exact rate
that was used. For a refund/reversal, call `convert` with the direction flipped
(`to_original, from_original, same_date`) — if only the forward row exists, the inverse fallback reciprocates
it automatically (`inverse = true`); do NOT register a second, independent `to→from` rate to "support" the
reverse direction — it drifts from the original and injects a phantom FX gain/loss. To register a rate, call
`upsert_rate`, not the generic CRUD `create`/`upsert` on `CurrencyExchange` — only `upsert_rate` enforces the
overlap-refusal that keeps `convert` deterministic (the DB CHECK is the last-resort backstop, not the
primary contract). Corporate never mutates your tables and posts no GL.

## Not a contract
- The 12 generated CRUD endpoints per entity are convenience scaffolding. Writing a `CurrencyExchange` row
  through the generic `create`/`upsert`/`bulk_create` endpoints skips the overlap check `upsert_rate`
  performs — you're relying on the DB EXCLUDE/CHECK constraints alone. Prefer `upsert_rate`.
- `// <<< CUSTOM` blocks preserve local edits only; not a cross-module extension point.

## Invariants a consumer must not break
- Never fabricate a rate when `convert` returns `NoRate` — register one via `upsert_rate` first.
- Never hand-register a `to→from` row to "complete" a pair that already has a `from→to` row for the reverse
  direction — the inverse fallback already reciprocates the existing row exactly; a second row drifts.
- A rate row is immutable history once other documents have stamped its `rate_id`; change the rate going
  forward with a new, adjacent (non-overlapping) window, not by editing the old one.
- Corporate posts no GL and calls no other module — if you find yourself importing another module's write
  path from inside corporate, that logic belongs in the consumer, not here.
