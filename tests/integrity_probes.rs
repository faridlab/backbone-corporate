//! Integrity probes — the FX invariants: a missing rate is a hard signal (never a guess), overlapping
//! windows are refused (the maturity invariant — an unambiguous rate), and bad rate input is rejected.

mod common;
use common::*;

use backbone_corporate::application::service::fx_service::*;
use uuid::Uuid;

// FIP-1 — no rate for the pair/date → NoRate, NOT a silent 1:1 or a zero. A consumer must not book a
// foreign amount at a guessed rate.
#[tokio::test]
async fn fip1_missing_rate_is_a_hard_signal() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let svc = FxService::new(pool.clone());
    let r = svc.convert(Some(Uuid::new_v4()), dec("100"), "USD", "IDR", d(2020, 1, 1)).await;
    assert!(matches!(r, Err(FxError::NoRate { .. })), "no rate must be an error, not a guess");
}

// FIP-2 — MATURITY: two OVERLAPPING effective windows for one directed pair cannot both exist. `upsert_rate`
// refuses the overlap, so `convert` for any historical date matches at most one row — a deterministic,
// reproducible conversion. Proven-by-revert: dropping the overlap guard lets both rows land, and `convert`
// then picks one by undefined order (a past transaction re-translating to a different number).
#[tokio::test]
async fn fip2_overlapping_windows_refused() {
    let pool = pool().await;
    let from = fake_currency(&pool, 2).await;
    let to = fake_currency(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();

    svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: from.clone(), to_currency: to.clone(),
        rate: dec("15000"), effective_from: d(2026, 1, 1), effective_to: Some(d(2026, 6, 30)),
    }).await.unwrap();

    // A second window that OVERLAPS the first (2026-06 is inside both) — must be refused.
    let clash = svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: from.clone(), to_currency: to.clone(),
        rate: dec("16000"), effective_from: d(2026, 6, 1), effective_to: None,
    }).await;
    assert!(matches!(clash, Err(FxError::OverlappingWindow { .. })), "an ambiguous rate window is refused");

    // The one surviving window still resolves deterministically.
    let out = svc.convert(Some(company), dec("1"), &from, &to, d(2026, 6, 15)).await.unwrap();
    assert_eq!(out.rate, dec("15000"));
}

// FIP-3 — a non-overlapping ADJACENT window is allowed (the legitimate rate-change case) — the guard bounds
// overlap, not succession.
#[tokio::test]
async fn fip3_adjacent_windows_allowed() {
    let pool = pool().await;
    let from = fake_currency(&pool, 2).await;
    let to = fake_currency(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();
    svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: from.clone(), to_currency: to.clone(),
        rate: dec("15000"), effective_from: d(2025, 1, 1), effective_to: Some(d(2025, 12, 31)),
    }).await.unwrap();
    let ok = svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: from.clone(), to_currency: to.clone(),
        rate: dec("16000"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await;
    assert!(ok.is_ok(), "a rate change (adjacent window) is the normal case");
}

// FIP-4 — bad rate input: a non-positive rate and a same-currency pair are rejected on write.
#[tokio::test]
async fn fip4_bad_rate_input_rejected() {
    let pool = pool().await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();
    let zero = svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "IDR".into(),
        rate: dec("0"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await;
    assert!(matches!(zero, Err(FxError::Invalid(_))), "a zero/negative rate is refused");
    let same = svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "USD".into(),
        rate: dec("1"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await;
    assert!(matches!(same, Err(FxError::Invalid(_))), "a self-pair is refused");
}

// FIP-5 — MATURITY: the positivity invariant is enforced at the DB, not only in `upsert_rate`. A `rate=0`
// row inserted through ANY other writer (the generic CRUD stack, a raw INSERT) would make `convert` return
// amount 0 WITH an audit stamp — money silently destroyed, worse than a NoRate. The `rate > 0` CHECK closes
// that trust boundary. Proven-by-revert: dropping the CHECK lets the row land and convert returns 0.
#[tokio::test]
async fn fip5_zero_rate_rejected_at_db() {
    let pool = pool().await;
    let from = fake_currency(&pool, 2).await;
    let to = fake_currency(&pool, 0).await;
    // A raw INSERT — the same trust boundary the CRUD create/upsert/bulk endpoints sit on (they never call
    // `upsert_rate`). The DB CHECK must reject a non-positive rate here.
    let bad = sqlx::query(
        r#"INSERT INTO corporate.currency_exchanges (id, company_id, from_currency, to_currency, rate, effective_from)
           VALUES (gen_random_uuid(), $1, $2, $3, 0, DATE '2026-01-01')"#,
    )
    .bind(Uuid::new_v4()).bind(&from).bind(&to)
    .execute(&pool).await;
    assert!(bad.is_err(), "a zero rate cannot be inserted through any writer — the CHECK backstops it");

    let neg = sqlx::query(
        r#"INSERT INTO corporate.currency_exchanges (id, company_id, from_currency, to_currency, rate, effective_from)
           VALUES (gen_random_uuid(), $1, $2, $3, -16250, DATE '2026-01-01')"#,
    )
    .bind(Uuid::new_v4()).bind(&from).bind(&to)
    .execute(&pool).await;
    assert!(neg.is_err(), "a negative rate (receivable→payable flip) cannot be inserted");
}
