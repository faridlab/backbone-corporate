# backbone-corporate — business flows & golden cases

## Flow: a consumer converts a foreign amount and books it
```
consumer holds a foreign amount + a transaction date (e.g. a USD supplier bill dated 2026-06-01)
   │
   ▼  FxService::convert(company_id, amount, from, to, on_date)
   │        │
   │        ├─ from == to → identity (rate 1, no row)
   │        │
   │        ├─ direct lookup: a from→to row effective on on_date
   │        │     (company-scoped wins over global; overlap is prevented on write → deterministic)
   │        │     → round to `to`'s decimal_places → Converted { amount, rate, rate_id, inverse:false }
   │        │
   │        ├─ no direct row, but a to→from row exists (the reversal case)
   │        │     → reciprocate that SAME row → Converted { rate: 1/fwd_rate, rate_id:<fwd row>, inverse:true }
   │        │
   │        └─ neither exists → NoRate (a hard signal — never a guess)
   │
   └▶ the CALLER stamps `rate` + `rate_id` on its own document and posts in its functional currency
```
Posts NO GL — corporate never writes another module's tables; it is read, never a caller.

## Golden cases (`tests/fx_golden_cases.rs`)
- **FGC-1 — convert rounds to the quote currency.** 12.34 USD × 16250.5 → rounds to IDR's 0 decimal places
  (200,531), not fractional rupiah.
- **FGC-2 — same-currency is identity.** No rate row needed; rate 1, amount unchanged.
- **FGC-3 — effective-dating reproduces history.** A 2025 rate and a 2026 rate (adjacent windows); a 2025
  document converts at the 2025 rate, a 2026 document at the 2026 rate — history never re-translates.
- **FGC-4 — company rate overrides global.** A negotiated company rate wins over the global default for the
  same pair/date; another company falls back to the global rate.
- **FGC-5 — convert returns the rate for stamping.** The exact `rate_id` used is returned, so the consumer
  can stamp it on its own transaction (the audit/revaluation record every foreign-currency document owes).
- **FGC-6 — the inverse reciprocates the stamped row.** Only `USD→IDR` is registered; converting the IDR
  amount back to USD reciprocates the SAME forward row (same `rate_id`, `inverse=true`) and nets to the
  minor unit exactly (1000.00 USD, not a drifted 999.xx). This is what lets `backbone-payment`'s
  `reverse_payment` on a foreign receipt work without a hand-typed, drifting inverse rate. Proven-by-revert
  (completeness council 2026-07-12).

## Integrity probes (`tests/integrity_probes.rs`)
- **FIP-1 — missing rate is a hard signal.** No rate covers the pair/date → `NoRate`, never a silent 1:1 or
  zero.
- **FIP-2 — overlapping windows refused (the maturity invariant).** Two overlapping effective windows for
  one directed pair cannot both exist — `upsert_rate` refuses the second, so `convert` for any historical
  date matches at most one row. Proven-by-revert: dropping the overlap guard lets both rows land and
  `convert` picks nondeterministically.
- **FIP-3 — adjacent windows allowed.** A non-overlapping, adjacent window (the legitimate rate-change
  case) is accepted — the guard bounds overlap, not succession.
- **FIP-4 — bad rate input rejected (app layer).** A non-positive rate and a self-pair (`from == to`) are
  refused by `upsert_rate`.
- **FIP-5 — zero/negative rate rejected at the DB.** A raw `INSERT` bypassing `upsert_rate` (the same trust
  boundary the generic CRUD `create`/`upsert`/`bulk_create` endpoints sit on) with `rate = 0` or `rate =
  -16250` is rejected by the `CHECK (rate > 0)` constraint. Proven-by-revert: dropping the CHECK lets the
  rows land and `convert` returns `amount = 0` (money destroyed, with an audit stamp) or a negative amount
  (receivable→payable flip) — worse than an error. (maturity council 2026-07-12)

## Seam (`tests/fx_accounting_seam.rs`)
- **FXSEAM-1 — a converted foreign bill posts balanced in the REAL ledger.** A USD 100 supplier bill
  converts through corporate's real FX engine at the effective rate (16,250) → IDR 1,625,000, then posts as
  a balanced journal (Dr Expense 1,625,000 · Cr A/P 1,625,000) accepted by the REAL
  `backbone-accounting` `PostingService`. `backbone-accounting` is a dev-dependency only — corporate itself
  posts no GL; this proves the number corporate produces is bookable, not that corporate books it.

## §5 round-trip (`scripts/fx_seam_roundtrip.sh`)
Regen (`--force`) leaves the seam files byte-identical; the oracle + seam re-run green.
