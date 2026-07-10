//! Shared test helpers: a live pool, currency seeding, and per-test isolation for the shared rate table.
//!
//! The `currency_exchanges` EXCLUDE constraint spans the WHOLE table, so parallel tests must not write
//! overlapping windows for the same (pair, company) scope. Isolation strategy: every test uses a unique
//! `company_id` and scopes its rates to it (distinct company scopes never collide); tests that exercise a
//! GLOBAL (null-company) rate use a fresh fake currency pair so their null-company window is unique too.

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
        r#"INSERT INTO corporate.currencies (id, iso_code, name, decimal_places, is_active)
           VALUES (gen_random_uuid(),'IDR','Indonesian Rupiah',0,true),
                  (gen_random_uuid(),'USD','US Dollar',2,true)
           ON CONFLICT (iso_code) WHERE (metadata->>'deleted_at') IS NULL DO NOTHING"#,
    )
    .execute(pool).await.expect("seed std currencies");
}

/// Seed one currency with an explicit code + minor-unit precision. Returns the code.
pub async fn currency(pool: &PgPool, iso: &str, name: &str, decimal_places: i32) -> String {
    sqlx::query(
        r#"INSERT INTO corporate.currencies (id, iso_code, name, decimal_places, is_active)
           VALUES (gen_random_uuid(),$1,$2,$3,true)
           ON CONFLICT (iso_code) WHERE (metadata->>'deleted_at') IS NULL DO NOTHING"#,
    )
    .bind(iso).bind(name).bind(decimal_places)
    .execute(pool).await.expect("seed currency");
    iso.to_string()
}

/// A fresh fake currency (unique 3-char code, given precision) so a global-rate test's window is unique.
pub async fn fake_currency(pool: &PgPool, decimal_places: i32) -> String {
    let code = Uuid::new_v4().simple().to_string()[..3].to_uppercase();
    currency(pool, &code, "Fake", decimal_places).await
}
