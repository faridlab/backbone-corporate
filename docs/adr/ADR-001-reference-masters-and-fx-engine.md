# ADR-001 — Reference masters, effective-dated FX, and the positivity/inverse fixes

Status: accepted · 2026-07-12 · Platform (Tier 5; posts no GL)

## Context
Every module that prices, invoices, or books a foreign-currency document needs the same reference masters:
what currencies exist, at what rate they convert, what a delivery term or commercial territory means.
`backbone-corporate` owns them so other modules cite them as logical FKs instead of re-inventing them. The
load-bearing piece is the FX engine — the rest (Territory, Incoterm, TermsAndConditions) are thin masters
with no behavior beyond being referenced. Corporate posts no GL and never calls another module; consumers
read it.

## Decision
1. **A rate is directional and effective-dated, not a symmetric spot price.** `1 from_currency = rate
   to_currency`, valid over `[effective_from, effective_to]`. `convert` picks the rate effective ON the
   transaction date, so a document booked in the past always reproduces the same number — history is never
   silently re-translated by a later rate change.
2. **Overlapping windows for one directed pair are refused — the determinism invariant.** `upsert_rate`
   checks for an overlap in the same tx as the insert; the DB carries an EXCLUDE backstop. Verified sound
   (maturity council 2026-07-12): the app's inclusive `a1<=b2 AND a2<=b1` and the DB's
   `daterange(from,to,'[]')` agree byte-for-byte at every boundary — no off-by-one.
3. **A company-scoped rate overrides a global one** for the same pair/date — the negotiated-rate case.
4. **Rate positivity is enforced at the DB, not only in the service (maturity council 2026-07-12).**
   `CurrencyExchange` auto-wires the generic 12-endpoint CRUD stack; those endpoints never call
   `upsert_rate`, and the schema's `@non_negative`/`@precision(20,10)` were never emitted to DDL. A `rate =
   0` row inserted through `create`/`upsert`/`bulk_create` would make `convert` return `amount = 0` **with
   an audit stamp** — silent money destruction, worse than an error. A negative rate flips
   receivable↔payable. Fixed with `CHECK (rate > 0)` on `corporate.currency_exchanges`
   (migration `20260712000200`) — closes every writer at the trust boundary that matters.
5. **A refund un-books the exact stamped rate via the same row's reciprocal — not a market inverse
   (completeness council 2026-07-12).** A rate is directional, so `convert(to, from, date)` hard-failed
   `NoRate` when only the `from→to` row existed. A built consumer — `backbone-payment`'s `reverse_payment`,
   a `backbone-billing` credit note — needs to answer "16,250,000 IDR remits back to how many USD?" for a
   receipt stamped at `USD→IDR = 16250`. The trap: hand-registering the opposite `IDR→USD` row stores an
   INDEPENDENT rate that drifts from the original and injects a phantom FX gain/loss. The fix: when the
   direct lookup misses, retry the reverse-direction row and return its RECIPROCAL
   (`rate_id` = the forward row, `inverse = true`) — the same stamped row, not a second registered one. A
   generic bidirectional market convert was explicitly rejected (a market inverse carries its own spread).

## Consequences
- A historical document's converted amount is provably reproducible and provably positive, through every
  writer. A refund round-trips to the minor unit against the exact original rate. Proven vs REAL
  `backbone-accounting` (a converted bill posts balanced); survives regen (§5).

## Parking lot (each with a gate)
- **A zero/negative rate through the CRUD stack silently zeroed or flipped a converted amount** — FIXED
  (maturity council 2026-07-12): `CHECK (rate > 0)` at the DB (FIP-5, proven-by-revert).
- **A foreign-currency refund had no way to un-book the exact stamped rate** — FIXED (completeness council
  2026-07-12): the inverse fallback reciprocates the same forward row (FGC-6, proven-by-revert).
- **Unknown/soft-deleted quote currency defaults to 2 dp** in `decimal_places` — a deleted IDR row would
  mis-round to 2 places. Gate: a currency-active check on `convert`, or a NOT-NULL FK from the rate to a
  live currency.
- **`rate NUMERIC` unbounded scale** — `amount × rate` could exceed rust_decimal's 28-digit envelope for
  very large IDR figures. Gate: emit the schema's `@precision(20,10)` scale to DDL.
- **Revaluation / consolidation reporting** hits the same missing-direction shape at scale, but no such
  reporting consumer is built — speculative where the refund path is concrete. Gate: a
  reporting/consolidation module.
- **`convert_at(rate_id, amount)`** — a consumer holding a stamped `rate_id` re-converting without a date
  lookup. Gate: a revaluation consumer that stores rate ids.
