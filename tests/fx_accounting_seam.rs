//! The FX seam against the REAL backbone-accounting ledger. Corporate is the multi-currency prerequisite:
//! it holds the effective-dated rate; a consumer converts a foreign amount and books it in the functional
//! currency (IDR). This test plays that consumer — convert a USD supplier bill through corporate's real FX
//! engine, then post the resulting IDR amount as a BALANCED journal in the REAL ledger. Proves the number
//! corporate produces lands, balanced, in accounting. ZERO normal Cargo edge — accounting is a dev-dep only;
//! corporate never posts GL.

mod common;
use common::*;

use backbone_accounting::application::service::posting_service::{PostingLine, PostingRequest, PostingService};
use backbone_corporate::application::service::fx_service::*;
use rust_decimal::Decimal;
use uuid::Uuid;

async fn account(pool: &sqlx::PgPool, company: Uuid, code: &str, atype: &str, subtype: &str, normal: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, company_id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_header, is_detail, status)
           VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                   false,true,'active'::account_status)"#,
    )
    .bind(id).bind(company).bind(code).bind(code).bind(code).bind(atype).bind(subtype).bind(normal)
    .execute(pool).await.expect("seed account");
    id
}
async fn balance(pool: &sqlx::PgPool, acct: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(debit_amount),0) - COALESCE(SUM(credit_amount),0) FROM accounting.ledgers WHERE account_id=$1")
        .bind(acct).fetch_one(pool).await.expect("balance")
}

// FXSEAM-1 — a USD 100 supplier bill, converted at the effective rate (16,250), posts a balanced IDR
// journal (Dr Expense 1,625,000 · Cr A/P 1,625,000) accepted by the REAL ledger.
#[tokio::test]
async fn fxseam1_converted_foreign_bill_posts_balanced() {
    let pool = pool().await;
    seed_std_currencies(&pool).await;
    let company = Uuid::new_v4();
    let fx = FxService::new(pool.clone());
    fx.upsert_rate(NewRate {
        company_id: Some(company), from_currency: "USD".into(), to_currency: "IDR".into(),
        rate: dec("16250"), effective_from: d(2026, 1, 1), effective_to: None,
    }).await.unwrap();

    // The consumer converts the foreign amount through corporate's real FX engine.
    let converted = fx.convert(Some(company), dec("100"), "USD", "IDR", d(2026, 6, 1)).await.unwrap();
    assert_eq!(converted.amount, dec("1625000"), "USD 100 @ 16,250 = IDR 1,625,000");

    // …and books it in the REAL ledger, in the functional currency, balanced.
    let expense = account(&pool, company, "6100-COR", "expense", "operating_expense", "debit").await;
    let ap = account(&pool, company, "2100-COR", "liability", "accounts_payable", "credit").await;
    let svc = PostingService::new(pool.clone());
    let mut req = PostingRequest::original(company, "manual", Uuid::new_v4(), d(2026, 6, 1));
    req.source_reference = Some(format!("USD bill @ {}", converted.rate));
    req.lines = vec![
        PostingLine { account_id: expense, debit: converted.amount, credit: Decimal::ZERO,
            party_type: None, party_id: None, cost_center_id: None, project_id: None, department_id: None,
            description: Some("foreign supplier bill (converted)".into()) },
        PostingLine { account_id: ap, debit: Decimal::ZERO, credit: converted.amount,
            party_type: Some("supplier".into()), party_id: Some(Uuid::new_v4()),
            cost_center_id: None, project_id: None, department_id: None, description: None },
    ];
    svc.post(req, None).await.expect("the real ledger accepts the converted, balanced journal");

    assert_eq!(balance(&pool, expense).await, dec("1625000"));
    assert_eq!(balance(&pool, ap).await, dec("-1625000"));
    let net = balance(&pool, expense).await + balance(&pool, ap).await;
    assert_eq!(net, Decimal::ZERO, "double-entry: the converted amount balances");
}
