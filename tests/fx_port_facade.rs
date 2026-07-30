//! Port facade — proves the module's PUBLISHED FX contract works end-to-end through
//! `CorporateModule::fx_port()`, WITHOUT importing `application::service::fx_service`.
//! This is the boundary the contract seat demanded: a sibling module (or integration test)
//! converts and registers rates through the export-layer port, never reaching past `exports/`.

mod common;
use common::*;

use backbone_corporate::CorporateModule;
use backbone_corporate::exports::CorporateFxPort;
use backbone_corporate::exports::RegisterRate;
use std::sync::Arc;
use uuid::Uuid;

// FXPORT-1 — the published port round-trips: register a rate via the port, then convert via the
// port, getting back the contract `Converted` (amount + stamped rate). No application internals.
#[tokio::test]
async fn fxport1_port_facade_round_trips() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let module = CorporateModule::builder().with_database(pool.clone()).build().expect("build module");
    let port: Arc<dyn CorporateFxPort> = module.fx_port();
    let company = Uuid::new_v4();

    port.register_rate(RegisterRate {
        company_id: Some(company), from: "USD".into(), to: "IDR".into(),
        rate: dec("16250"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.expect("register rate via port");

    let c = port.convert(Some(company), dec("100"), "USD", "IDR", d(2026, 6, 1))
        .await.expect("convert via port");
    assert_eq!(c.amount, dec("1625000"), "USD 100 @ 16,250 = IDR 1,625,000 (0 dp)");
    assert_eq!(c.rate, dec("16250"));
    assert!(c.rate_id.is_some(), "the stamped rate row is returned for the audit record");
    assert!(!c.inverse, "a direct USD->IDR row is not an inverse conversion");
}

// FXPORT-2 — a missing rate surfaces through the port as an error (anyhow), NOT a panic or a
// silent default. The application's typed FxError is mapped at the boundary; the consumer sees a
// failure it must handle, never a guessed rate.
#[tokio::test]
async fn fxport2_missing_rate_is_an_error_via_port() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let module = CorporateModule::builder().with_database(pool.clone()).build().expect("build module");

    let r = module.fx_port().convert(Some(Uuid::new_v4()), dec("100"), "USD", "IDR", d(2020, 1, 1)).await;
    assert!(r.is_err(), "NoRate must surface through the port, not default or panic");
}
