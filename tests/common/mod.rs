//! Shared test helpers: a live pool, currency seeding, and per-test isolation for the shared rate table.
//!
//! Rates are global reference data — one table-wide window per directed pair, enforced by the
//! table-level EXCLUDE constraint. Isolation strategy: every test mints a FRESH fake currency
//! pair (`fx_pair`) so its windows can never collide with another test's, in this binary or any
//! other running in parallel.

#![allow(dead_code)]

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

pub fn dburl() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_corporate".into())
}
pub async fn pool() -> PgPool {
    PgPool::connect(&dburl()).await.expect("connect")
}
pub fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}
pub fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

/// Ensure the standard currencies exist (IDR 0 dp, USD 2 dp). Safe to call in every test.
pub async fn seed_std_currencies(pool: &PgPool) {
    sqlx::query(
        r#"INSERT INTO corporate.currencies (id, iso_code, name, decimal_places, status)
           VALUES (gen_random_uuid(),'IDR','Indonesian Rupiah',0,'active'),
                  (gen_random_uuid(),'USD','US Dollar',2,'active')
           ON CONFLICT (iso_code) WHERE (metadata->>'deleted_at') IS NULL DO NOTHING"#,
    )
    .execute(pool)
    .await
    .expect("seed std currencies");
}

/// Seed one currency with an explicit code + minor-unit precision. Returns the code.
pub async fn currency(pool: &PgPool, iso: &str, name: &str, decimal_places: i32) -> String {
    sqlx::query(
        r#"INSERT INTO corporate.currencies (id, iso_code, name, decimal_places, status)
           VALUES (gen_random_uuid(),$1,$2,$3,'active')
           ON CONFLICT (iso_code) WHERE (metadata->>'deleted_at') IS NULL DO NOTHING"#,
    )
    .bind(iso)
    .bind(name)
    .bind(decimal_places)
    .execute(pool)
    .await
    .expect("seed currency");
    iso.to_string()
}

/// A fresh fake currency (unique 3-char code, given precision) so a test's windows are unique.
pub async fn fake_currency(pool: &PgPool, decimal_places: i32) -> String {
    let code = Uuid::new_v4().simple().to_string()[..3].to_uppercase();
    currency(pool, &code, "Fake", decimal_places).await
}

/// A fresh directed currency pair unique to the calling test: (base, quote). The base gets
/// 2 minor units (a USD-like convention), the quote `quote_dp` — pass 0 for a whole-unit
/// quote (an IDR-like convention) to exercise rounding to the minor unit.
pub async fn fx_pair(pool: &PgPool, quote_dp: i32) -> (String, String) {
    let from = fake_currency(pool, 2).await;
    let to = fake_currency(pool, quote_dp).await;
    (from, to)
}
