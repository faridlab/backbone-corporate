//! FXS — the raw spot-rate read (`FxService::spot_on_or_before` / `CorporateFxPort`), the
//! latest-on-or-before contract the retired banking rate table served, carried onto corporate's
//! windowed rate table.
//!
//! - FXS-1: on a gapless chain (consecutive windows, last open) the read returns exactly the row
//!   that STARTED latest at-or-before the date — banking parity, and the shape a migration copies.
//! - FXS-2: a date before the first window refuses (NoRate) — nothing to fall back to.
//! - FXS-3: a GAP refuses rather than resurrecting a deliberately closed window's rate.
//! - FXS-4: a company row wins over a global row for a scoped caller; a platform (None) caller
//!   sees the global row.
//! - FXS-5: it is a RAW read — no reciprocal fallback, no rounding, no quote-precision contact.
//! - FXS-6: rate_type + source stamping on registration (defaults + explicit provenance).
//! - FXS-7: the published port carries the same read as the contract `SpotRate`.
//!
//! Requires DATABASE_URL (:5433/backbone_corporate with corporate migrated).

mod common;
use common::*;

use backbone_corporate::application::service::fx_service::*;
use backbone_corporate::domain::entity::RateType;
use uuid::Uuid;

/// FXS-1 — a gapless chain: A [Jan 1, Jun 30], B [Jul 1, Dec 31], C [Jan 1 2027, ∞). Every probe
/// date returns the row that started latest at-or-before it — including both window boundaries,
/// where the predecessor ends the day the successor starts, so no date matches two rows.
#[tokio::test]
async fn fxs1_gapless_chain_reads_the_latest_row_at_or_before() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = FxService::new(pool.clone());

    let a = svc
        .upsert_rate(NewRate {
            company_id: Some(company),
            from_currency: "USD".into(),
            to_currency: "IDR".into(),
            rate: dec("15000"),
            effective_from: d(2026, 1, 1),
            effective_to: Some(d(2026, 6, 30)),
            rate_type: RateType::Spot,
            source: None,
        })
        .await
        .unwrap();
    let b = svc
        .upsert_rate(NewRate {
            company_id: Some(company),
            from_currency: "USD".into(),
            to_currency: "IDR".into(),
            rate: dec("16000"),
            effective_from: d(2026, 7, 1),
            effective_to: Some(d(2026, 12, 31)),
            rate_type: RateType::Spot,
            source: None,
        })
        .await
        .unwrap();
    let c = svc
        .upsert_rate(NewRate {
            company_id: Some(company),
            from_currency: "USD".into(),
            to_currency: "IDR".into(),
            rate: dec("16250"),
            effective_from: d(2027, 1, 1),
            effective_to: None,
            rate_type: RateType::Spot,
            source: None,
        })
        .await
        .unwrap();

    // (probe date) → (expected row id, expected rate, expected effective_from)
    for (on, id, rate, eff) in [
        (d(2026, 1, 1), a, "15000", d(2026, 1, 1)), // the first day: A, by its own start
        (d(2026, 2, 15), a, "15000", d(2026, 1, 1)), // mid-A
        (d(2026, 6, 30), a, "15000", d(2026, 1, 1)), // A's last day
        (d(2026, 7, 1), b, "16000", d(2026, 7, 1)), // B's first day — the handover
        (d(2026, 12, 31), b, "16000", d(2026, 7, 1)), // B's last day
        (d(2027, 1, 1), c, "16250", d(2027, 1, 1)), // C's first day
        (d(2028, 5, 20), c, "16250", d(2027, 1, 1)), // deep into the open window
    ] {
        let s = svc
            .spot_on_or_before(Some(company), "USD", "IDR", on)
            .await
            .unwrap();
        assert_eq!(
            s.rate_id, id,
            "date {on}: the row that started latest at-or-before"
        );
        assert_eq!(s.rate, dec(rate), "date {on}: that row's rate, verbatim");
        assert_eq!(
            s.effective_from, eff,
            "date {on}: that row's effective_from"
        );
    }
}

/// FXS-2 — before the chain begins there is nothing at-or-before: refuse, never invent.
#[tokio::test]
async fn fxs2_before_the_first_window_refuses() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = FxService::new(pool.clone());
    svc.upsert_rate(NewRate {
        company_id: Some(company),
        from_currency: "USD".into(),
        to_currency: "IDR".into(),
        rate: dec("15000"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();

    let e = svc
        .spot_on_or_before(Some(company), "USD", "IDR", d(2025, 12, 31))
        .await
        .unwrap_err();
    assert!(matches!(e, FxError::NoRate { .. }), "got {e:?}");
}

/// FXS-3 — a gap between windows refuses: the closed window's rate is retired, and a raw read must
/// not resurrect it through the hole. (The latest-on-or-before read on the retired banking table
/// could not see holes — its rows had no end dates. Windows make retirement explicit.)
#[tokio::test]
async fn fxs3_a_gap_refuses_instead_of_resurrecting_a_closed_window() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let svc = FxService::new(pool.clone());
    svc.upsert_rate(NewRate {
        company_id: Some(company),
        from_currency: "USD".into(),
        to_currency: "IDR".into(),
        rate: dec("15000"),
        effective_from: d(2026, 1, 1),
        effective_to: Some(d(2026, 6, 30)),
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();
    svc.upsert_rate(NewRate {
        company_id: Some(company),
        from_currency: "USD".into(),
        to_currency: "IDR".into(),
        rate: dec("16000"),
        effective_from: d(2026, 8, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();

    // Inside the July hole: nothing covers it, and July 31 (the day before the next window) must
    // NOT return the June-closed 15,000.
    for on in [d(2026, 7, 1), d(2026, 7, 15), d(2026, 7, 31)] {
        let e = svc
            .spot_on_or_before(Some(company), "USD", "IDR", on)
            .await
            .unwrap_err();
        assert!(
            matches!(e, FxError::NoRate { .. }),
            "date {on}: a gap must refuse, got {e:?}"
        );
    }
    // Both sides of the hole still read their own rows.
    assert_eq!(
        svc.spot_on_or_before(Some(company), "USD", "IDR", d(2026, 6, 30))
            .await
            .unwrap()
            .rate,
        dec("15000")
    );
    assert_eq!(
        svc.spot_on_or_before(Some(company), "USD", "IDR", d(2026, 8, 1))
            .await
            .unwrap()
            .rate,
        dec("16000")
    );
}

/// FXS-4 — scope: a company row wins over a global row for the scoped caller, and the platform
/// (None-company) caller reads the global row. Fresh fake pair so the global window is unique.
#[tokio::test]
async fn fxs4_company_scope_wins_over_global_for_the_scoped_caller() {
    let pool = pool().await;
    let from = fake_currency(&pool, 2).await;
    let to = fake_currency(&pool, 0).await;
    let company = Uuid::new_v4();
    let svc = FxService::new(pool.clone());

    // The GLOBAL row (no company bound — the admin path registers it unscoped).
    svc.upsert_rate(NewRate {
        company_id: None,
        from_currency: from.clone(),
        to_currency: to.clone(),
        rate: dec("9999"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();
    // The COMPANY row, same window.
    svc.upsert_rate(NewRate {
        company_id: Some(company),
        from_currency: from.clone(),
        to_currency: to.clone(),
        rate: dec("1111"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();

    let scoped = svc
        .spot_on_or_before(Some(company), &from, &to, d(2026, 6, 1))
        .await
        .unwrap();
    assert_eq!(
        scoped.rate,
        dec("1111"),
        "the company row wins over the global one"
    );
    let platform = svc
        .spot_on_or_before(None, &from, &to, d(2026, 6, 1))
        .await
        .unwrap();
    assert_eq!(
        platform.rate,
        dec("9999"),
        "the platform caller reads the global row"
    );
}

/// FXS-5 — a RAW read, not a conversion: no reciprocal fallback (a to→from row does not serve a
/// from→to ask), and no rounding — a 10-dp rate comes back verbatim even when the quote currency
/// has 0 minor units (convert would round; spot never touches precision).
#[tokio::test]
async fn fxs5_raw_read_no_inverse_no_rounding() {
    let pool = pool().await;
    let from = fake_currency(&pool, 2).await;
    let to = fake_currency(&pool, 0).await;
    let company = Uuid::new_v4();
    let svc = FxService::new(pool.clone());

    // Only the FORWARD row of the reverse pair exists.
    svc.upsert_rate(NewRate {
        company_id: Some(company),
        from_currency: to.clone(),
        to_currency: from.clone(),
        rate: dec("0.000061532203"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();
    let e = svc
        .spot_on_or_before(Some(company), &from, &to, d(2026, 6, 1))
        .await
        .unwrap_err();
    assert!(
        matches!(e, FxError::NoRate { .. }),
        "no reciprocal fallback, got {e:?}"
    );

    // A full-precision direct rate reads back VERBATIM.
    svc.upsert_rate(NewRate {
        company_id: Some(company),
        from_currency: from.clone(),
        to_currency: to.clone(),
        rate: dec("16250.1234567890"),
        effective_from: d(2026, 1, 1),
        effective_to: None,
        rate_type: RateType::Spot,
        source: None,
    })
    .await
    .unwrap();
    let s = svc
        .spot_on_or_before(Some(company), &from, &to, d(2026, 6, 1))
        .await
        .unwrap();
    assert_eq!(
        s.rate,
        dec("16250.1234567890"),
        "the stored rate, unrounded"
    );
}

/// FXS-6 — provenance stamping: a default registration lands 'spot'/unstamped; an explicit one
/// lands its own rate_type + source on the row.
#[tokio::test]
async fn fxs6_rate_type_and_source_stamp_the_row() {
    let pool = pool().await;
    let from = fake_currency(&pool, 2).await;
    let to = fake_currency(&pool, 2).await;
    let company = Uuid::new_v4();
    let svc = FxService::new(pool.clone());

    let dflt = svc
        .upsert_rate(NewRate {
            company_id: Some(company),
            from_currency: from.clone(),
            to_currency: to.clone(),
            rate: dec("100"),
            effective_from: d(2026, 1, 1),
            effective_to: None,
            rate_type: RateType::Spot,
            source: None,
        })
        .await
        .unwrap();
    let stamped = svc
        .upsert_rate(NewRate {
            company_id: Some(company),
            from_currency: to.clone(),
            to_currency: from.clone(),
            rate: dec("0.01"),
            effective_from: d(2026, 1, 1),
            effective_to: None,
            rate_type: RateType::AvgPeriod,
            source: Some("manual".into()),
        })
        .await
        .unwrap();

    let (rt, src): (String, Option<String>) = sqlx::query_as(
        "SELECT rate_type::text, source FROM corporate.currency_exchanges WHERE id=$1",
    )
    .bind(dflt)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (rt.as_str(), src),
        ("spot", None),
        "default registration: spot, unstamped"
    );

    let (rt, src): (String, Option<String>) = sqlx::query_as(
        "SELECT rate_type::text, source FROM corporate.currency_exchanges WHERE id=$1",
    )
    .bind(stamped)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (rt.as_str(), src.as_deref()),
        ("avg_period", Some("manual")),
        "explicit registration stamps its own provenance"
    );
}

/// FXS-7 — the published port carries the same read as the contract `SpotRate` (and its default
/// registration is a spot row). Sibling modules must not reach into application internals.
#[tokio::test]
async fn fxs7_the_port_carries_the_contract_spot_rate() {
    use backbone_corporate::exports::CorporateFxPort;
    use backbone_corporate::CorporateModule;

    let pool = pool().await;
    let module = CorporateModule::builder()
        .with_database(pool.clone())
        .build()
        .expect("build module");
    let port = module.fx_port();
    let from = fake_currency(&pool, 2).await;
    let to = fake_currency(&pool, 2).await;
    let company = Uuid::new_v4();

    let id = port
        .register_rate(backbone_corporate::exports::RegisterRate {
            company_id: Some(company),
            from: from.clone(),
            to: to.clone(),
            rate: dec("15500"),
            effective_from: d(2026, 3, 1),
            effective_to: None,
            rate_type: None,
            source: None,
        })
        .await
        .expect("register via port");

    let s = port
        .spot_on_or_before(Some(company), &from, &to, d(2026, 9, 9))
        .await
        .expect("spot via port");
    assert_eq!(s.rate, dec("15500"));
    assert_eq!(s.rate_id, id, "the port names the row it read");
    assert_eq!(s.effective_from, d(2026, 3, 1));

    // The None-rate_type registration landed as a spot row.
    let rt: String =
        sqlx::query_scalar("SELECT rate_type::text FROM corporate.currency_exchanges WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rt, "spot", "absent rate_type means a plain spot rate");
}
