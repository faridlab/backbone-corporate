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
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum FxError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invalid input: {0}")]
    Invalid(String),
    /// No rate covers the requested pair on the requested date — the consumer must not guess one.
    #[error("no rate: {from}->{to} on {date}")]
    NoRate { from: String, to: String, date: NaiveDate },
    /// The new window overlaps an existing rate for the same directed pair — a rate must be unambiguous.
    #[error("overlapping rate window: {from}->{to} overlaps an existing effective window")]
    OverlappingWindow { from: String, to: String },
    /// The quote (or source) currency is not a known active row in `corporate.currencies`. Returning a
    /// silent 2-dp default for an unknown code would mis-round monetary amounts (ADR-001 parking lot).
    #[error("unknown currency: {0}")]
    UnknownCurrency(String),
}

pub struct NewRate {
    pub company_id: Option<Uuid>,
    pub from_currency: String,
    pub to_currency: String,
    pub rate: Decimal,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
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
                return Err(FxError::Invalid("effective_to before effective_from".into()));
            }
        }

        let mut tx = self.pool.begin().await?;
        // RLS fence (ADR-0008): bind the caller's company onto the transaction so the fence's USING
        // clause lets the overlap-check SELECT see this company's rows (and, on INSERT, the WITH CHECK
        // accepts the row). A NULL company_id (a GLOBAL rate) is NOT bound here: the currency_exchanges
        // policy writes own-only, so a global rate must be created via the admin/bypass path. Binding
        // would let a tenant forge a global rate, which is exactly what the fence exists to prevent.
        //
        // `set_config(_, _, true)` is transaction-local, so the setting is discarded on commit/rollback
        // and can never ride a pooled connection into the next request.
        if let Some(company) = r.company_id {
            bind_company_tx(&mut *tx, company).await?;
        }
        // Overlap check within the same company scope. Two windows [a1,b1] and [a2,b2] overlap iff
        // a1 <= b2 AND a2 <= b1, with a null end treated as +infinity.
        let overlap: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM corporate.currency_exchanges
               WHERE from_currency=$1 AND to_currency=$2
                 AND company_id IS NOT DISTINCT FROM $3
                 AND (metadata->>'deleted_at') IS NULL
                 AND effective_from <= COALESCE($5, DATE '9999-12-31')
                 AND $4 <= COALESCE(effective_to, DATE '9999-12-31')
               LIMIT 1"#,
        )
        .bind(&from).bind(&to).bind(r.company_id).bind(r.effective_from).bind(r.effective_to)
        .fetch_optional(&mut *tx).await?;
        if overlap.is_some() {
            return Err(FxError::OverlappingWindow { from, to });
        }

        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO corporate.currency_exchanges
                 (id, company_id, from_currency, to_currency, rate, effective_from, effective_to)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
        )
        .bind(id).bind(r.company_id).bind(&from).bind(&to).bind(r.rate)
        .bind(r.effective_from).bind(r.effective_to)
        .execute(&mut *tx).await?;
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
            return Ok(Converted { amount, rate: Decimal::ONE, rate_id: None, rate_date: on_date, inverse: false });
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
            bind_company_tx(&mut *tx, company).await?;
        }

        let dp = decimal_places_on(&mut *tx, &to).await?;

        // Direct lookup: prefer a company rate over a global one, then the most recently-effective window.
        // Deterministic — overlap is prevented on write, so at most one window per scope covers the date;
        // the ORDER BY only chooses between company vs global.
        if let Some((rate, rate_id)) = lookup_rate_on(&mut *tx, &from, &to, on_date).await? {
            let converted = (amount * rate).round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero);
            tx.commit().await?;
            return Ok(Converted { amount: converted, rate, rate_id: Some(rate_id), rate_date: on_date, inverse: false });
        }

        // Inverse fallback: no direct `from→to` row, but a `to→from` row exists — apply its RECIPROCAL. A
        // foreign-currency refund/reversal must un-book the EXACT stamped rate, so we reciprocate the same
        // registered row (rate_id points at the forward row) rather than a separately-registered inverse
        // that would drift from it. This is the narrow reversal case — NOT a generic bidirectional market
        // convert.
        if let Some((fwd_rate, fwd_id)) = lookup_rate_on(&mut *tx, &to, &from, on_date).await? {
            let rate = Decimal::ONE / fwd_rate;
            let converted = (amount / fwd_rate).round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero);
            tx.commit().await?;
            return Ok(Converted { amount: converted, rate, rate_id: Some(fwd_id), rate_date: on_date, inverse: true });
        }

        tx.commit().await?;
        Err(FxError::NoRate { from, to, date: on_date })
    }
}

/// Bind `company` onto `conn` transaction-local via `set_config('app.company_id', ..., true)` — the
/// `true` scopes the setting to the surrounding transaction, so it is discarded on commit/rollback and
/// cannot leak onto a pooled connection reused by the next request. This is the same fence the ORM's
/// `company_scope::bind_company_on` applies; it is inlined here because the framework `main` pin this
/// crate resolves against does not yet export that module (ADR-0008 follow-up — switch to
/// `backbone_orm::company_scope::bind_company_on` once the framework release propagates).
async fn bind_company_tx(conn: &mut sqlx::PgConnection, company: Uuid) -> Result<(), FxError> {
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(company.to_string())
        .execute(conn)
        .await?;
    Ok(())
}

/// The effective rate (+ its row id) for a directed pair on a date, or None. Company scope wins over
/// global; among a scope the most recent window wins (overlap is prevented on write). Runs on `conn`
/// (a transaction that already carries `app.company_id` when scoped); the SQL predicate keeps the
/// company-wins-over-global ordering explicit on the application side as defense-in-depth.
async fn lookup_rate_on(
    conn: &mut sqlx::PgConnection,
    from: &str,
    to: &str,
    on_date: NaiveDate,
) -> Result<Option<(Decimal, Uuid)>, FxError> {
    let row = sqlx::query(
        r#"SELECT id, rate FROM corporate.currency_exchanges
           WHERE from_currency=$1 AND to_currency=$2
             AND (company_id IS NOT DISTINCT FROM NULLIF(current_setting('app.company_id', true), '')::uuid
                  OR company_id IS NULL)
             AND (metadata->>'deleted_at') IS NULL
             AND effective_from <= $3
             AND (effective_to IS NULL OR effective_to >= $3)
           ORDER BY (company_id IS NOT NULL) DESC, effective_from DESC
           LIMIT 1"#,
    )
    .bind(from).bind(to).bind(on_date)
    .fetch_optional(conn).await?;
    Ok(row.map(|r| (r.get::<Decimal, _>("rate"), r.get::<Uuid, _>("id"))))
}

/// The quote currency's minor-unit precision (IDR=0, USD=2). Errors on an unknown or soft-deleted
/// currency rather than silently defaulting to 2 dp — a deleted or typo'd code would otherwise
/// mis-round monetary amounts (ADR-001 parking lot). Runs on `conn` because the caller already opened
/// the scoped transaction; `corporate.currencies` is NOT itself company-fenced.
async fn decimal_places_on(conn: &mut sqlx::PgConnection, iso: &str) -> Result<u32, FxError> {
    let dp: Option<i32> = sqlx::query_scalar(
        "SELECT decimal_places FROM corporate.currencies WHERE iso_code=$1 AND (metadata->>'deleted_at') IS NULL")
        .bind(iso).fetch_optional(conn).await?;
    dp.map(|v| v.max(0) as u32)
        .ok_or_else(|| FxError::UnknownCurrency(iso.to_string()))
}

fn norm(iso: &str) -> Result<String, FxError> {
    let t = iso.trim().to_uppercase();
    if t.len() < 3 || t.len() > 3 {
        return Err(FxError::Invalid(format!("currency code must be 3 letters: {iso:?}")));
    }
    Ok(t)
}
