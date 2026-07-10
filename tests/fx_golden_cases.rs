//! Golden cases — the FX-engine oracle: an amount converts at the rate effective on the transaction date,
//! rounded to the quote currency; a same-currency conversion is the identity; effective-dating reproduces
//! history; a company rate overrides a global one; the rate used is returned for stamping.

mod common;
use common::*;

use backbone_corporate::application::service::fx_service::*;
use uuid::Uuid;

// FGC-1 — convert USD→IDR at the effective rate, rounded to IDR's 0 minor units.
#[tokio::test]
async fn fgc1_convert_rounds_to_quote_currency() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();
    svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "IDR".into(),
        rate: dec("16250.5"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.unwrap();

    // 12.34 USD × 16250.5 = 200,531.17 → IDR rounds to 0 dp → 200,531.
    let out = svc.convert(Some(company), dec("12.34"), "USD", "IDR", d(2026, 3, 1)).await.unwrap();
    assert_eq!(out.amount, dec("200531"), "rounded to IDR's 0 decimal places");
    assert_eq!(out.rate, dec("16250.5"));
    assert!(out.rate_id.is_some());
}

// FGC-2 — a same-currency conversion is the identity (rate 1, amount unchanged), no rate row needed.
#[tokio::test]
async fn fgc2_same_currency_is_identity() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let svc = FxService::new(pool.clone());
    let out = svc.convert(None, dec("999.99"), "IDR", "IDR", d(2026, 3, 1)).await.unwrap();
    assert_eq!(out.amount, dec("999.99"));
    assert_eq!(out.rate, dec("1"));
    assert!(out.rate_id.is_none());
}

// FGC-3 — effective-dating: a historical date picks the OLD rate; a later date picks the NEW rate. The two
// windows are adjacent (not overlapping), so both coexist and history reproduces its booked number.
#[tokio::test]
async fn fgc3_effective_dating_reproduces_history() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();
    svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "IDR".into(),
        rate: dec("15000"), effective_from: d(2025, 1, 1), effective_to: Some(d(2025, 12, 31)),
    }).await.unwrap();
    svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "IDR".into(),
        rate: dec("16250"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.unwrap();

    let old = svc.convert(Some(company), dec("100"), "USD", "IDR", d(2025, 6, 1)).await.unwrap();
    let new = svc.convert(Some(company), dec("100"), "USD", "IDR", d(2026, 6, 1)).await.unwrap();
    assert_eq!(old.amount, dec("1500000"), "the 2025 document reproduces the 2025 rate");
    assert_eq!(new.amount, dec("1625000"), "the 2026 document uses the current rate");
}

// FGC-4 — a company-scoped rate overrides a global (null-company) rate for the same pair + date.
#[tokio::test]
async fn fgc4_company_rate_overrides_global() {
    let pool = pool().await;
    let from = fake_currency(&pool, 2).await; // fresh pair so the global window is unique across tests
    let to = fake_currency(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();
    // Global rate for everyone…
    svc.upsert_rate(NewRate {
        company_id: None, from_currency: from.clone(), to_currency: to.clone(),
        rate: dec("100"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.unwrap();
    // …and a negotiated company rate for the same pair/date.
    svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: from.clone(), to_currency: to.clone(),
        rate: dec("110"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.unwrap();

    let mine = svc.convert(Some(company), dec("10"), &from, &to, d(2026, 6, 1)).await.unwrap();
    let anyone = svc.convert(Some(Uuid::new_v4()), dec("10"), &from, &to, d(2026, 6, 1)).await.unwrap();
    assert_eq!(mine.rate, dec("110"), "my negotiated rate wins");
    assert_eq!(anyone.rate, dec("100"), "another company falls back to the global rate");
}

// FGC-5 — the conversion returns the rate + rate row it used, so the consumer can STAMP it on the
// transaction (the audit/revaluation record a foreign-currency document owes). Completeness council.
#[tokio::test]
async fn fgc5_convert_returns_rate_for_stamping() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();
    let rate_id = svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "IDR".into(),
        rate: dec("16000"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.unwrap();

    let out = svc.convert(Some(company), dec("50"), "USD", "IDR", d(2026, 6, 1)).await.unwrap();
    assert_eq!(out.amount, dec("800000"));
    assert_eq!(out.rate, dec("16000"));
    assert_eq!(out.rate_id, Some(rate_id), "the exact rate row is returned to stamp on the document");
    assert_eq!(out.rate_date, d(2026, 6, 1));
}

// FGC-6 — inverse round-trip (completeness council): a foreign-currency REFUND must un-book the exact
// stamped rate. Only USD→IDR is registered; converting the IDR amount back to USD reciprocates the SAME
// forward row (same rate_id) and nets to the minor unit — so backbone-payment's `reverse_payment` on a
// foreign receipt lands 1000.00 USD, not a drifted 999.xx from a hand-typed inverse row.
#[tokio::test]
async fn fgc6_inverse_reciprocates_the_stamped_row() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let svc = FxService::new(pool.clone());
    let company = Uuid::new_v4();
    let fwd_id = svc.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "IDR".into(),
        rate: dec("16250"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.unwrap();

    // The original receipt: 1000 USD → 16,250,000 IDR (the number booked + stamped).
    let fwd = svc.convert(Some(company), dec("1000"), "USD", "IDR", d(2026, 6, 1)).await.unwrap();
    assert_eq!(fwd.amount, dec("16250000"));
    assert!(!fwd.inverse);

    // The refund: convert the IDR amount back to USD — no direct IDR→USD row exists; the reciprocal of the
    // SAME forward row is used, so it round-trips exactly and carries the forward row's id.
    let back = svc.convert(Some(company), dec("16250000"), "IDR", "USD", d(2026, 6, 1)).await.unwrap();
    assert_eq!(back.amount, dec("1000.00"), "the refund un-books the exact original amount");
    assert!(back.inverse, "produced from the reciprocal of the forward row");
    assert_eq!(back.rate_id, Some(fwd_id), "the SAME stamped row — not a drifting hand-typed inverse");
}
