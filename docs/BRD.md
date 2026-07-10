# backbone-corporate — BRD

## Documents
Currency (the money master) · CurrencyExchange (an effective-dated directional FX rate) · Territory (the
commercial sales tree) · Incoterm · TermsAndConditions. Own Postgres schema `corporate`. Posts **no GL**.
Corporate never calls another module; consumers read it through a `ConversionPort`.

## Business rules

**BR-1 (directional, effective-dated rate).** A `CurrencyExchange` row means `1 from_currency =
rate to_currency`, valid over `[effective_from, effective_to]` (an open `effective_to` = the current rate).
`convert(company_id, amount, from, to, on_date)` picks the row effective **on the transaction date**, so a
document booked in the past always reproduces the same number regardless of when it's re-viewed.

**BR-2 (unambiguous rate — the maturity invariant).** For one directed pair + company scope, effective
windows must not overlap — `convert` for a historical date must match at most one row, never pick
nondeterministically between two. `upsert_rate` refuses an overlapping window; the DB has an EXCLUDE
backstop that agrees with the app's inclusive-overlap check byte-for-byte at every boundary.

**BR-3 (company rate overrides global).** A rate with `company_id = NULL` is a global default; a rate scoped
to a company for the same pair/date wins over it. This is the negotiated-rate case (a company gets a better
FX deal than the market default).

**BR-4 (rate must be strictly positive — the DB is the trust boundary).** A rate of zero or negative is
refused. This is enforced at the DB (`CHECK (rate > 0)` on `corporate.currency_exchanges`), not only in
`upsert_rate`, because `CurrencyExchange` also auto-wires the generic 12-endpoint CRUD stack
(`create`/`upsert`/`bulk_create`), which never calls `upsert_rate`. A `rate = 0` row would make `convert`
return `amount = 0` **with an audit stamp** — money silently destroyed, worse than an error, because the
document looks fully audited. A negative rate flips a receivable into a payable (maturity council
2026-07-12).

**BR-5 (round to the quote currency's precision).** A converted amount is rounded to the `to_currency`'s
`decimal_places` (IDR = 0, USD = 2), using midpoint-away-from-zero rounding.

**BR-6 (inverse fallback for reversal — not a market inverse).** When no direct `from→to` row covers the
date but a `to→from` row does, `convert` returns the **reciprocal of that same row** (`inverse = true`,
`rate_id` = the forward row's id) instead of failing. This exists so a foreign-currency refund
(`backbone-payment`'s `reverse_payment`, a `backbone-billing` credit note) un-books the *exact* rate that
was stamped on the original document. A separately hand-registered `to→from` row would drift from the
original and inject a phantom FX gain/loss — deliberately not supported (completeness council 2026-07-12).

**BR-7 (same-currency identity).** Converting a currency to itself is the identity: rate 1, no rate row
needed, `rate_id = None`.

**BR-8 (missing rate is a hard signal).** If neither a direct nor an invertible rate covers the pair/date,
`convert` returns `NoRate` — never a guessed 1:1 or a zero. A consumer must not book a foreign amount at a
fabricated rate.

## Events
None. Corporate posts no GL and emits no domain events — it is a read-mostly reference layer.

## Deferred (with reason)
Concrete rate-feed provider adapters, revaluation/consolidation reporting, per-region default currency
profiles beyond IDR (PRD).
