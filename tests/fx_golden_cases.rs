//! Golden cases — the FX-engine oracle: an amount converts at the rate effective on the transaction date,
//! rounded to the quote currency; a same-currency conversion is the identity; effective-dating reproduces
//! history; the rate used is returned for stamping.

mod common;
use common::*;

use backbone_corporate::application::service::fx_service::*;
use backbone_corporate::domain::entity::RateType;

// FGC-1 — convert base→quote at the effective rate, rounded to the quote currency's 0 minor units.
#[tokio::test]
async fn fgc1_convert_rounds_to_quote_currency() {
    let pool = pool().await;
    let (from, to) = fx_pair(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    svc.upsert_rate(NewRate {
        from_currency: from.clone(),
        to_currency: to.clone(),
        rate: dec("16250.5"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();

    // 12.34 base × 16250.5 = 200,531.17 → 0-dp quote rounds to 200,531.
    let out = svc
        .convert(dec("12.34"), &from, &to, d(2026, 3, 1))
        .await
        .unwrap();
    assert_eq!(
        out.amount,
        dec("200531"),
        "rounded to the quote's 0 decimal places"
    );
    assert_eq!(out.rate, dec("16250.5"));
    assert!(out.rate_id.is_some());
}

// FGC-2 — a same-currency conversion is the identity (rate 1, amount unchanged), no rate row needed.
#[tokio::test]
async fn fgc2_same_currency_is_identity() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let svc = FxService::new(pool.clone());
    let out = svc
        .convert(dec("999.99"), "IDR", "IDR", d(2026, 3, 1))
        .await
        .unwrap();
    assert_eq!(out.amount, dec("999.99"));
    assert_eq!(out.rate, dec("1"));
    assert!(out.rate_id.is_none());
}

// FGC-3 — effective-dating: a historical date picks the OLD rate; a later date picks the NEW rate. The two
// windows are adjacent (not overlapping), so both coexist and history reproduces its booked number.
#[tokio::test]
async fn fgc3_effective_dating_reproduces_history() {
    let pool = pool().await;
    let (from, to) = fx_pair(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    svc.upsert_rate(NewRate {
        from_currency: from.clone(),
        to_currency: to.clone(),
        rate: dec("15000"),
        effective_from: d(2025, 1, 1),
        effective_to: Some(d(2025, 12, 31)),
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();
    svc.upsert_rate(NewRate {
        from_currency: from.clone(),
        to_currency: to.clone(),
        rate: dec("16250"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();

    let old = svc
        .convert(dec("100"), &from, &to, d(2025, 6, 1))
        .await
        .unwrap();
    let new = svc
        .convert(dec("100"), &from, &to, d(2026, 6, 1))
        .await
        .unwrap();
    assert_eq!(
        old.amount,
        dec("1500000"),
        "the 2025 document reproduces the 2025 rate"
    );
    assert_eq!(
        new.amount,
        dec("1625000"),
        "the 2026 document uses the current rate"
    );
}

// FGC-4 — the conversion returns the rate + rate row it used, so the consumer can STAMP it on the
// transaction (the audit/revaluation record a foreign-currency document owes). Completeness council.
#[tokio::test]
async fn fgc4_convert_returns_rate_for_stamping() {
    let pool = pool().await;
    let (from, to) = fx_pair(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    let rate_id = svc
        .upsert_rate(NewRate {
            from_currency: from.clone(),
            to_currency: to.clone(),
            rate: dec("16000"),
            effective_from: d(2026, 1, 1),
            effective_to: None,
            rate_type: RateType::Spot,
            source: None,
        })
        .await
        .unwrap();

    let out = svc
        .convert(dec("50"), &from, &to, d(2026, 6, 1))
        .await
        .unwrap();
    assert_eq!(out.amount, dec("800000"));
    assert_eq!(out.rate, dec("16000"));
    assert_eq!(
        out.rate_id,
        Some(rate_id),
        "the exact rate row is returned to stamp on the document"
    );
    assert_eq!(out.rate_date, d(2026, 6, 1));
}

// FGC-5 — inverse round-trip (completeness council): a foreign-currency REFUND must un-book the exact
// stamped rate. Only base→quote is registered; converting the quote amount back reciprocates the SAME
// forward row (same rate_id) and nets to the minor unit — so backbone-payment's `reverse_payment` on a
// foreign receipt lands 1000.00 base, not a drifted 999.xx from a hand-typed inverse row.
#[tokio::test]
async fn fgc5_inverse_reciprocates_the_stamped_row() {
    let pool = pool().await;
    let (from, to) = fx_pair(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    let fwd_id = svc
        .upsert_rate(NewRate {
            from_currency: from.clone(),
            to_currency: to.clone(),
            rate: dec("16250"),
            effective_from: d(2026, 1, 1),
            effective_to: None,
            rate_type: RateType::Spot,
            source: None,
        })
        .await
        .unwrap();

    // The original receipt: 1000 base → 16,250,000 quote (the number booked + stamped).
    let fwd = svc
        .convert(dec("1000"), &from, &to, d(2026, 6, 1))
        .await
        .unwrap();
    assert_eq!(fwd.amount, dec("16250000"));
    assert!(!fwd.inverse);

    // The refund: convert the quote amount back to base — no direct quote→base row exists; the reciprocal
    // of the SAME forward row is used, so it round-trips exactly and carries the forward row's id.
    let back = svc
        .convert(dec("16250000"), &to, &from, d(2026, 6, 1))
        .await
        .unwrap();
    assert_eq!(
        back.amount,
        dec("1000.00"),
        "the refund un-books the exact original amount"
    );
    assert!(
        back.inverse,
        "produced from the reciprocal of the forward row"
    );
    assert_eq!(
        back.rate_id,
        Some(fwd_id),
        "the SAME stamped row — not a drifting hand-typed inverse"
    );
}

// FGC-6 — overflow safety: an amount near rust_decimal's 28-digit ceiling, times a large rate,
// overflows the envelope. This must surface as FxError::Overflow, NOT a panic from naive `*`.
#[tokio::test]
async fn fgc6_overflow_is_an_error_not_a_panic() {
    let pool = pool().await;
    let (from, to) = fx_pair(&pool, 0).await;
    let svc = FxService::new(pool.clone());
    svc.upsert_rate(NewRate {
        from_currency: from.clone(),
        to_currency: to.clone(),
        rate: dec("16250"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();

    // 28 nines (~1e28, under Decimal::MAX so it parses) x 16,250 overflows the 28-digit envelope.
    let huge = dec("9999999999999999999999999999");
    let r = svc.convert(huge, &from, &to, d(2026, 6, 1)).await;
    assert!(
        matches!(r, Err(FxError::Overflow)),
        "overflow must be a typed error, got {:?}",
        r
    );
}
