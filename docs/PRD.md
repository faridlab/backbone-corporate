# backbone-corporate — PRD

Platform (Tier 5) · the **reference-master layer** · posts no GL · the multi-currency prerequisite.

## Why
Every module that prices, invoices, or books a foreign-currency document needs the same handful of
reference masters: what currencies exist and at what rate they convert, what a delivery term means, what
territory a deal belongs to. `backbone-corporate` owns those masters — Currency, CurrencyExchange
(effective-dated FX rates), Territory (the commercial sales tree), Incoterm, TermsAndConditions — so
selling/buying/accounting cite them as logical FKs instead of re-inventing them. The load-bearing piece is
the FX engine: convert an amount at the rate **effective on the transaction date**, so a historical document
always reproduces the number it was booked with. Corporate posts no GL and never calls another module;
consumers READ it through a `ConversionPort`.

## Scope (KEEP)
- **Currency** — the ISO 4217 money master (code, name, symbol, `decimal_places` for minor-unit rounding,
  active flag).
- **CurrencyExchange** — a directional, effective-dated rate (`1 from_currency = rate to_currency`),
  optionally company-scoped (a negotiated rate overrides the global default).
- **The FX engine (`FxService`)** — `convert` picks the rate effective on the transaction date, rounds to
  the quote currency's precision, and returns the rate + rate row used (for stamping on the document);
  `upsert_rate` registers a rate, refusing an overlapping effective window for a directed pair so conversion
  stays deterministic. An inverse fallback lets a refund un-book the exact stamped rate when only the
  forward row exists.
- **Territory** — a commercial sales tree (parent/child), the dimension selling/CRM cite for
  territory-based pricing, targets, and reporting. NOT the administrative wilayah (that's `backbone-geo`).
- **Incoterm / TermsAndConditions** — thin reference masters stamped on orders/quotes/invoices; no
  behavior beyond being referenced.

## Non-goals (CUT / DEFER)
- Concrete rate-feed provider adapters (a bank/market API that pushes live rates) — a consuming service
  calls `upsert_rate`; corporate doesn't fetch rates itself.
- Revaluation / consolidation reporting (presenting IDR-functional books in a foreign parent's currency) —
  no such reporting consumer is built yet; the missing direction is the same as the refund case, but
  speculative until one exists.
- Per-region default currency profiles beyond IDR.
- Tax rules — stay in `backbone-tax`.

## Success criteria
- A historical document always reconverts to the same number: `convert` is deterministic because
  overlapping rate windows for one directed pair are refused at both the app layer and the DB.
- A rate can never be zero or negative through ANY writer (generic CRUD included), so a foreign invoice can
  never silently book as zero or flip receivable↔payable.
- A foreign-currency refund converts back through the exact rate that was stamped on the original document,
  not a drifting, separately-registered inverse.
- A converted amount posts, balanced, in the REAL `backbone-accounting` ledger (proven against the REAL
  module). Zero normal Cargo edge; survives a full codegen regen (§5). Posts no GL.
