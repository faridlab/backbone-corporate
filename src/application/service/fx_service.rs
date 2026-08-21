//! The hand-authored FX conversion engine (user-owned; survives regen).
//!
//! Corporate is the reference-master layer — currencies + the effective-dated rate table. The load-bearing
//! logic is `convert`: translate an amount from one currency to another at the rate **effective on the
//! transaction date**, so historical documents reproduce the number they were booked with. A rate is
//! DIRECTIONAL (1 `from` = `rate` × `to`) and effective-dated; a rate change coexists with history.
//!
//! The maturity invariant is that a rate must be UNAMBIGUOUS: for one directed pair (+ company scope) the
//! effective windows must not overlap, or `convert` for a historical date would match two rows and pick one
//! nondeterministically — the same past transaction re-translating to a different number run-to-run.
//! `upsert_rate` rejects an overlapping window; the DB has an EXCLUDE backstop.
//!
//! Posts NO GL. Corporate never calls another module; consumers read it (a `ConversionPort`).

use chrono::NaiveDate;
use rust_decimal::prelude::*;
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    CurrencyExchangeRepository, CurrencyRepository, NewCurrencyExchangeRow,
};

#[derive(Debug, thiserror::Error)]
pub enum FxError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invalid input: {0}")]
    Invalid(String),
    /// No rate covers the requested pair on the requested date — the consumer must not guess one.
    #[error("no rate: {from}->{to} on {date}")]
    NoRate {
        from: String,
        to: String,
        date: NaiveDate,
    },
    /// The new window overlaps an existing rate for the same directed pair — a rate must be unambiguous.
    #[error("overlapping rate window: {from}->{to} overlaps an existing effective window")]
    OverlappingWindow { from: String, to: String },
    /// The quote (or source) currency is not a known active row in `corporate.currencies`. Returning a
    /// silent 2-dp default for an unknown code would mis-round monetary amounts (ADR-001 parking lot).
    #[error("unknown currency: {0}")]
    UnknownCurrency(String),
    /// The conversion overflowed `rust_decimal`'s 28-digit envelope (e.g. a very large IDR amount
    /// multiplied by a big rate). Surfaced as a typed error so a caller can handle it instead of the
    /// process panicking on the naive `*` / `/`.
    #[error("arithmetic overflow in conversion")]
    Overflow,
}

pub struct NewRate {
    pub company_id: Option<Uuid>,
    pub from_currency: String,
    pub to_currency: String,
    pub rate: Decimal,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    /// What the rate is (a point-in-time spot by default; period averages/close rates carry their
    /// own convention). Stamped on the row so a historical document can say WHICH kind produced it.
    pub rate_type: crate::domain::entity::RateType,
    /// Where the rate came from (e.g. "manual", "migrated_banking") — provenance for audit. None
    /// means unstamped, not "unknown source"; bulk rollovers stamp their rows explicitly.
    pub source: Option<String>,
}

/// The result of a conversion — the amount AND the rate that produced it, so a consumer can STAMP the rate
/// on its transaction (the audit/revaluation record every foreign-currency document owes).
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub amount: Decimal,
    pub rate: Decimal,
    /// The rate row used (None on a same-currency identity conversion). On an inverse conversion this is the
    /// id of the FORWARD row whose reciprocal was applied — so a refund un-books the exact stamped rate.
    pub rate_id: Option<Uuid>,
    pub rate_date: NaiveDate,
    /// True when the amount was produced from the reciprocal of a `to→from` row (no direct row existed).
    pub inverse: bool,
}

pub struct FxService {
    pool: PgPool,
}

impl FxService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Register a rate for a directed pair, rejecting a window that overlaps an existing one (same pair +
    /// same company scope). This is what keeps `convert` deterministic.
    pub async fn upsert_rate(&self, r: NewRate) -> Result<Uuid, FxError> {
        let from = norm(&r.from_currency)?;
        let to = norm(&r.to_currency)?;
        if from == to {
            return Err(FxError::Invalid("from and to currency are the same".into()));
        }
        if r.rate <= Decimal::ZERO {
            return Err(FxError::Invalid("rate must be positive".into()));
        }
        if let Some(end) = r.effective_to {
            if end < r.effective_from {
                return Err(FxError::Invalid(
                    "effective_to before effective_from".into(),
                ));
            }
        }
        if let Some(src) = &r.source {
            if src.chars().count() > 60 {
                return Err(FxError::Invalid("source longer than 60 chars".into()));
            }
        }

        let mut tx = self.pool.begin().await?;
        // RLS fence (ADR-0008): bind the caller's company onto the transaction so the fence's USING
        // clause lets the overlap-check SELECT see this company's rows (and, on INSERT, the WITH CHECK
        // accepts the row). A NULL company_id (a GLOBAL rate) is NOT bound here: the currency_exchanges
        // policy writes own-only, so a global rate must be created via the admin/bypass path. Binding
        // would let a tenant forge a global rate, which is exactly what the fence exists to prevent.
        if let Some(company) = r.company_id {
            backbone_orm::company_scope::bind_company_on(&mut *tx, company).await?;
        }
        let exchanges = CurrencyExchangeRepository::new(self.pool.clone());
        // Overlap check within the same company scope (see CurrencyExchangeRepository::find_overlap_tx).
        let overlap = exchanges
            .find_overlap_tx(
                &mut *tx,
                &from,
                &to,
                r.company_id,
                r.effective_from,
                r.effective_to,
            )
            .await?;
        if overlap.is_some() {
            return Err(FxError::OverlappingWindow { from, to });
        }

        let id = Uuid::new_v4();
        exchanges
            .insert_rate_tx(
                &mut *tx,
                &NewCurrencyExchangeRow {
                    id,
                    company_id: r.company_id,
                    from_currency: from.clone(),
                    to_currency: to.clone(),
                    rate: r.rate,
                    effective_from: r.effective_from,
                    effective_to: r.effective_to,
                    rate_type: r.rate_type,
                    source: r.source,
                },
            )
            .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Convert `amount` from → to at the rate effective on `on_date`, rounded to the quote currency's
    /// minor-unit precision. A same-currency conversion is the identity (rate 1). A company-scoped rate
    /// wins over a global (company_id IS NULL) rate; among candidates the most recent `effective_from` wins.
    pub async fn convert(
        &self,
        company_id: Option<Uuid>,
        amount: Decimal,
        from_currency: &str,
        to_currency: &str,
        on_date: NaiveDate,
    ) -> Result<Converted, FxError> {
        let from = norm(from_currency)?;
        let to = norm(to_currency)?;
        if from == to {
            return Ok(Converted {
                amount,
                rate: Decimal::ONE,
                rate_id: None,
                rate_date: on_date,
                inverse: false,
            });
        }

        // RLS fence (ADR-0008): `corporate.currency_exchanges` is company-fenced. With `app.company_id`
        // unset the fence shows ZERO rows to a non-super role — so an FX read on a scoped connection
        // returned NoRate even when rates existed, breaking every multi-currency consumer (Phase 4 F1).
        //
        // Fix: run the whole read path in ONE transaction, bind `app.company_id` onto it transaction-local
        // when the caller is scoped, so the fence's USING clause admits this company's rows AND the
        // global (NULL-company) fallback rows. The setting is transaction-scoped (`set_config(..., true)`),
        // so it is discarded on commit and can never ride a pooled connection into the next request. A
        // `None` company (platform caller) leaves the setting unset; under the non-super role that sees
        // only the global rows (USING `company_id IS NULL`).
        let mut tx = self.pool.begin().await?;
        if let Some(company) = company_id {
            backbone_orm::company_scope::bind_company_on(&mut *tx, company).await?;
        }

        let exchanges = CurrencyExchangeRepository::new(self.pool.clone());
        let currencies = CurrencyRepository::new(self.pool.clone());

        let dp = match currencies.decimal_places_tx(&mut *tx, &to).await? {
            Some(v) => v.max(0) as u32,
            None => return Err(FxError::UnknownCurrency(to)),
        };

        // Direct lookup: prefer a company rate over a global one, then the most recently-effective window.
        // Deterministic — overlap is prevented on write, so at most one window per scope covers the date;
        // the ORDER BY only chooses between company vs global.
        if let Some((rate, rate_id, _effective_from)) = exchanges
            .find_effective_rate_tx(&mut *tx, &from, &to, on_date)
            .await?
        {
            let prod = amount.checked_mul(rate).ok_or(FxError::Overflow)?;
            let converted = prod.round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero);
            tx.commit().await?;
            return Ok(Converted {
                amount: converted,
                rate,
                rate_id: Some(rate_id),
                rate_date: on_date,
                inverse: false,
            });
        }

        // Inverse fallback: no direct `from→to` row, but a `to→from` row exists — apply its RECIPROCAL. A
        // foreign-currency refund/reversal must un-book the EXACT stamped rate, so we reciprocate the same
        // registered row (rate_id points at the forward row) rather than a separately-registered inverse
        // that would drift from it. This is the narrow reversal case — NOT a generic bidirectional market
        // convert.
        if let Some((fwd_rate, fwd_id, _effective_from)) = exchanges
            .find_effective_rate_tx(&mut *tx, &to, &from, on_date)
            .await?
        {
            let rate = Decimal::ONE
                .checked_div(fwd_rate)
                .ok_or(FxError::Overflow)?;
            let q = amount.checked_div(fwd_rate).ok_or(FxError::Overflow)?;
            let converted = q.round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero);
            tx.commit().await?;
            return Ok(Converted {
                amount: converted,
                rate,
                rate_id: Some(fwd_id),
                rate_date: on_date,
                inverse: true,
            });
        }

        tx.commit().await?;
        Err(FxError::NoRate {
            from,
            to,
            date: on_date,
        })
    }

    /// The spot rate in force for a directed pair at (or immediately before) a date — a RAW rate
    /// read, not a conversion: no amount, no rounding, no inverse fallback, no same-currency
    /// identity. Carries the latest-on-or-before contract the retired banking rate table served:
    /// on a gapless chain (consecutive windows, last one open) the row returned is exactly the one
    /// that started latest at-or-before the date, which is what a migration parity probe asserts
    /// row-for-row. A gap refuses with `NoRate` — the consumer must not guess a retired rate.
    ///
    /// Scope follows `convert`: the caller's company is bound transaction-local so the fence admits
    /// own rows AND global fallback rows, with a company row winning over a global one.
    pub async fn spot_on_or_before(
        &self,
        company_id: Option<Uuid>,
        from_currency: &str,
        to_currency: &str,
        on_or_before: NaiveDate,
    ) -> Result<SpotRate, FxError> {
        let from = norm(from_currency)?;
        let to = norm(to_currency)?;

        let mut tx = self.pool.begin().await?;
        if let Some(company) = company_id {
            backbone_orm::company_scope::bind_company_on(&mut *tx, company).await?;
        }
        let exchanges = CurrencyExchangeRepository::new(self.pool.clone());
        let found = exchanges
            .find_effective_rate_tx(&mut *tx, &from, &to, on_or_before)
            .await?;
        tx.commit().await?;
        match found {
            Some((rate, rate_id, effective_from)) => Ok(SpotRate {
                rate,
                rate_id,
                effective_from,
            }),
            None => Err(FxError::NoRate {
                from,
                to,
                date: on_or_before,
            }),
        }
    }
}

/// A raw spot-rate read: the rate, the row it came from, and the date that row became effective —
/// enough for a consumer to stamp a historical document with exactly the rate it reproduces.
#[derive(Debug, Clone, PartialEq)]
pub struct SpotRate {
    pub rate: Decimal,
    pub rate_id: Uuid,
    pub effective_from: NaiveDate,
}

// (bind_company_tx / lookup_rate_on / decimal_places_on used to live here as inlined SQL. They moved
// into CurrencyExchangeRepository / CurrencyRepository, and the company bind now uses the framework's
// backbone_orm::company_scope::bind_company_on — the service no longer holds SQL.)
fn norm(iso: &str) -> Result<String, FxError> {
    let t = iso.trim().to_uppercase();
    if t.len() < 3 || t.len() > 3 {
        return Err(FxError::Invalid(format!(
            "currency code must be 3 letters: {iso:?}"
        )));
    }
    Ok(t)
}

// The published FX facade. Lives in this user-owned file (not in the generated
// `impl CorporateModule` block in lib.rs, which has no CUSTOM marker) so it
// survives `metaphor schema generate`. Sibling modules and integration tests
// convert/register through this port instead of reaching into application
// internals — the contract boundary the module owes its consumers.
impl crate::CorporateModule {
    /// The published FX contract. This is the sanctioned way for consumers to
    /// convert amounts and register rates; it returns the export-layer
    /// `CorporateFxPort`, decoupled from this file's `FxService`/`FxError`.
    pub fn fx_port(&self) -> std::sync::Arc<dyn crate::exports::CorporateFxPort> {
        std::sync::Arc::new(crate::exports::CorporateFxServiceImpl::new(
            self.fx_service.clone(),
        ))
    }
}
