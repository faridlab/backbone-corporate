---
date: 2026-07-31
repo_type: module
unit: backbone-corporate
focus: maturity
roster:
  standing: [chair, skeptic, steelman, yagni-business]
  context: [ddd-bounded-context, contract-seat]
  invited: [domain-expert]
  subagent_seats: [skeptic, steelman, chair]
---

# Council — module:backbone-corporate — focus: MATURITY

## Best call
Publish a `CorporateFxPort` trait in `src/exports/services.rs` (surface `convert` + `upsert_rate` returning export-layer DTOs) backed by `FxService`, AND wire `FxService` into the `CorporateModule` builder as a sixth field with a public accessor. One move — it resolves the four-way convergence (Steelman Condition 1 = Skeptic's load-bearing assumption = Contract Seat's attack = YAGNI's point) and makes the existing FXSEAM-1 seam test stop lying about which boundary it exercises.

- **Residual negative value:** ~0.5-1 engineering day to design the port DTOs (rate input, `Converted` output, `FxError` mapping) and re-route the seam test through the facade. The port becomes a *published* contract the day the first sibling adopts it — ~1-2 sprints of locked-in shape before you can refactor it cheaply again. Does NOT fix the DDD wound (generic write CRUD still mounted on the rate table) nor the deactivated-currency reproducibility hole — both stay open and bounded.
- **Reversibility:** easy. The engine already exists; this is additive surface area. The only one-way-ish part is post-first-consumer: once backbone-accounting imports `CorporateFxPort`, removing a method breaks it.
- **What would flip this:** evidence that direct construction (`FxService::new(pool)`) is the *documented* Metaphor platform contract for cross-module services — i.e. a sibling module or the framework docs explicitly prescribe constructing peer-module services by reaching past the facade. That would make the current state intentional, not unfinished, and the Best call would drop to "gate the write CRUD surface" (DDD seat's fix). I checked the cited lines; nothing in `lib.rs` or `exports/` suggests this is the documented pattern — the empty CUSTOM blocks read as "not yet done."

## Disagreement map
- **Reachability is part of the maturity bar at Tier-5** — Steelman says 4/5 mature because every ADR-001 clause maps to proven code; Skeptic/Contract/YAGNI say "not mature" because `convert` exists only on an internal service neither fielded on the facade nor on the exported trait, and the green suite proves internals, not the contract. **Crux:** whether a capability reachable only by reaching past the exports boundary counts as "complete" at the Tier-5 reference-master bar. It does not — the module's stated job is "other modules READ it," and there is no published way to do so for FX. Steelman's own Condition 1 concedes this; the Skeptic is right.
- **Cleanliness of the rate-table write surface** — Steelman says generic CRUD on `currency_exchanges` is harmless because the DB EXCLUDE/CHECK backstops money-correctness; DDD says two bounded contexts (editable master vs. deterministic effective-dated rate) are collapsed into one entity and generic write CRUD is the smell. **Crux:** is the cleanliness wound worth treating now or after the port. DDD's fix (default the rate table to `create_readonly_corporate_routes`) is cheap and correct, but lower leverage than the port — parking it behind the port is fine.

## Recommendations (ranked by leverage)
| # | Move | Leverage | Residual negative | Reversibility | Evidence to flip |
|---|------|----------|-------------------|---------------|------------------|
| 1 | Publish `CorporateFxPort` in `exports/services.rs` (convert + upsert_rate, export-layer DTOs) AND wire `FxService` into `CorporateModule::builder` as a public field; re-route FXSEAM-1 through the port. | high | ~0.5-1 day; published-shape lock-in after first consumer (~1-2 sprints). | easy (additive; one-way only post-first-consumer) | Documented Metaphor contract that peer services are constructed directly — would make the current state intentional. |
| 2 | Default `currency_exchanges` to `create_readonly_corporate_routes` (routes/mod.rs:59); route all rate mutation through `upsert_rate` (+ future `close_rate`). | med | Admin tooling/seeding must use a trusted path explicitly; minor UX cost. | easy | Evidence that downstream admin tooling depends on the open generic write surface today. |
| 3 | Stamp `decimal_places` onto the rate row / booked document at write time so historical conversion is independent of live currency-master state. | med | Touches schema + migration + booking surface; wider blast radius than the port. | costly (migration of booked rows) | Discovery that no historical document ever needs re-derivation after a quote currency retires — but ADR-001's stated reason-to-exist is reproducibility, so unlikely. |
| 4 | Leave direct construction as the documented contract; do nothing. | low | Every day unpublished, the first consumer reaches into `application::service::fx_service` and that becomes the de-facto contract — application-layer struct, not a designed port. Coupling cost compounds per consumer. | one-way door (consumers hardcode the internal path) | None — this is the reject path. Skeptic, Contract Seat, and YAGNI all converge against it. |

## Maturity scorecard
| Seat | Axis | Score (1-5) | One sentence why |
|------|------|-------------|------------------|
| DDD / bounded context | ddd-bounded-context | 3 | Two contexts (editable master vs. deterministic effective-dated rate) collapsed into one `CurrencyExchange` entity with generic write CRUD mounted on the rate table; DB EXCLUDE/CHECK backstops correctness so it's a cleanliness wound, not active corruption. |
| Contract | published-interface | 2 | The headline FX capability (`convert`, `upsert_rate`) exists only on an internal `FxService` that is neither a field on `CorporateModule` nor a method on the exported `CorporateQueryService` trait; the one real consumer reaches past the exports boundary. |
| Domain expert (FX) | fx-domain-fidelity | 4 | Engine implements every ADR-001 clause (directional+effective-dated, no-overlap, company-over-global, positivity, reciprocal inverse); the one divergence — historical reproducibility breaks when the quote currency is later deactivated — is real but acknowledged in ADR-001's own parking lot. |
| (added) FX engine correctness | fx-engine-correctness | 5 | Every ADR-001 clause maps to concrete code (fx_service.rs:121-181, 172-177, 102-107; migrations 20260712000100/200; currency_exchange_repository.rs:125) and is proven by money-correctness probes (FIP-2/FIP-5 revert, FGC-6 refund round-trip, FXSEAM-1 real-ledger zero-balance journal). |

**Net maturity:** engine internals 5/5, module contract 2/5. The headline capability is mature code wrapped in an immature module. The Tier-5 reference-master bar is not yet met — not because the engine is wrong, but because the contract is unpublished.

## Parking lot
- **Deleted-currency mis-rounding** (ADR-001 parking lot) — raised by Domain Expert / Steelman Condition 3; `decimal_places_tx` resolves precision regardless of active state, so a soft-deleted IDR row mis-rounds to 2 dp. Currently gated, not fixed. Scope: root (FX engine).
- **`NUMERIC` unbounded scale vs `rust_decimal`'s 28-digit envelope** on large IDR figures — raised by Steelman Condition 3; rate scale overflow risk. Scope: root (FX engine + schema).
- **Missing `schema/models/currency_exchange.model.yaml`** — raised by Steelman Condition 2; FX scaffolding cannot be re-derived from SSoT, survives regen only because it's user-owned. Scope: root (schema). Needs a decision: add the model YAML or formally mark the FX surface as user-owned in the schema manifest.
- **Uncommitted versioning WIP** — `git status` shows `src/presentation/versioning/{mod,version,version_compat,version_middleware,version_router}.rs` modified and unstaged. Scope: root. Out of maturity focus for the FX engine but relevant to the module's overall "complete and committed" bar.
- **Revaluation, consolidation, `convert_at`, true bidirectional market pricing** — raised by Domain Expert (secondary) / Steelman Condition 4; correctly NOT built or gated as speculative scope. Scope: root. Keep parked until a real consumer articulates the need.

---

### Relevant file paths
- `src/lib.rs` (lines 59-65: 5-field `CorporateModule`, no `fx_service`)
- `src/exports/services.rs` (lines 23-69: 15 read-only trait methods, no `convert`; lines 75-77: empty CUSTOM SERVICES block)
- `src/application/service/fx_service.rs` (the engine internals; not on the facade)
- `src/routes/mod.rs` (line 59: `create_readonly_corporate_routes` — the DDD seat's smallest fix target)
- `migrations/` (`20260712000100` EXCLUDE backstop, `20260712000200` positivity CHECK)
- `tests/fx_accounting_seam.rs` (line 15: import that reaches past the facade — the load-bearing seam)
