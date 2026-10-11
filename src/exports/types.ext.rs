//! The extension of `types.rs` (hand-written; ADR-0031).

#[allow(unused_imports)]
use super::*;

// Add custom public types here

// ----------------------------------------------------------------------------
// FX engine — the module's headline capability, exposed as CONTRACT types
// (not re-exports of application::service types). See CorporateFxPort.
// ----------------------------------------------------------------------------

/// The result of an FX conversion — the converted amount AND the rate that
/// produced it, so a consumer can STAMP the rate on its transaction (the
/// audit/revaluation record every foreign-currency document owes).
///
/// `inverse` is true when the amount came from the RECIPROCAL of a `to->from`
/// row (a refund un-booking the exact stamped forward rate); `rate_id` then
/// names that forward row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Converted {
    pub amount: Decimal,
    pub rate: Decimal,
    pub rate_id: Option<Uuid>,
    pub rate_date: NaiveDate,
    pub inverse: bool,
}

/// Request to register a directed, effective-dated rate:
/// `1 from = rate * to`, valid over `[effective_from, effective_to]`.
/// Rates are shared reference data — one table-wide rate per directed pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRate {
    pub from: String,
    pub to: String,
    pub rate: Decimal,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    /// What the rate is; absent means a plain spot rate.
    #[serde(default)]
    pub rate_type: Option<RateType>,
    /// Where the rate came from (provenance); absent means unstamped.
    #[serde(default)]
    pub source: Option<String>,
}

/// A raw spot-rate read: the rate in force at (or immediately before) a date,
/// the row it came from, and the date that row became effective. No amount is
/// converted and no rounding is applied — the consumer stamps, it does not
/// re-derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpotRate {
    pub rate: Decimal,
    pub rate_id: Uuid,
    pub effective_from: NaiveDate,
}
