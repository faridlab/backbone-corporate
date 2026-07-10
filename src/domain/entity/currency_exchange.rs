use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for CurrencyExchange
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CurrencyExchangeId(pub Uuid);

impl CurrencyExchangeId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for CurrencyExchangeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for CurrencyExchangeId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for CurrencyExchangeId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<CurrencyExchangeId> for Uuid {
    fn from(id: CurrencyExchangeId) -> Self { id.0 }
}

impl AsRef<Uuid> for CurrencyExchangeId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for CurrencyExchangeId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CurrencyExchange {
    pub id: Uuid,
    pub company_id: Option<Uuid>,
    pub from_currency: String,
    pub to_currency: String,
    pub rate: Decimal,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl CurrencyExchange {
    /// Create a builder for CurrencyExchange
    pub fn builder() -> CurrencyExchangeBuilder {
        CurrencyExchangeBuilder::default()
    }

    /// Create a new CurrencyExchange with required fields
    pub fn new(from_currency: String, to_currency: String, rate: Decimal, effective_from: NaiveDate) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id: None,
            from_currency,
            to_currency,
            rate,
            effective_from,
            effective_to: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> CurrencyExchangeId {
        CurrencyExchangeId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the company_id field (chainable)
    pub fn with_company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the effective_to field (chainable)
    pub fn with_effective_to(mut self, value: NaiveDate) -> Self {
        self.effective_to = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "from_currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.from_currency = v; }
                }
                "to_currency" => {
                    if let Ok(v) = serde_json::from_value(value) { self.to_currency = v; }
                }
                "rate" => {
                    if let Ok(v) = serde_json::from_value(value) { self.rate = v; }
                }
                "effective_from" => {
                    if let Ok(v) = serde_json::from_value(value) { self.effective_from = v; }
                }
                "effective_to" => {
                    if let Ok(v) = serde_json::from_value(value) { self.effective_to = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for CurrencyExchange {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "CurrencyExchange"
    }
}

impl backbone_core::PersistentEntity for CurrencyExchange {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for CurrencyExchange {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["from_currency", "to_currency"]
    }
}

/// Builder for CurrencyExchange entity
///
/// Provides a fluent API for constructing CurrencyExchange instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct CurrencyExchangeBuilder {
    company_id: Option<Uuid>,
    from_currency: Option<String>,
    to_currency: Option<String>,
    rate: Option<Decimal>,
    effective_from: Option<NaiveDate>,
    effective_to: Option<NaiveDate>,
}

impl CurrencyExchangeBuilder {
    /// Set the company_id field (optional)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the from_currency field (required)
    pub fn from_currency(mut self, value: String) -> Self {
        self.from_currency = Some(value);
        self
    }

    /// Set the to_currency field (required)
    pub fn to_currency(mut self, value: String) -> Self {
        self.to_currency = Some(value);
        self
    }

    /// Set the rate field (required)
    pub fn rate(mut self, value: Decimal) -> Self {
        self.rate = Some(value);
        self
    }

    /// Set the effective_from field (required)
    pub fn effective_from(mut self, value: NaiveDate) -> Self {
        self.effective_from = Some(value);
        self
    }

    /// Set the effective_to field (optional)
    pub fn effective_to(mut self, value: NaiveDate) -> Self {
        self.effective_to = Some(value);
        self
    }

    /// Build the CurrencyExchange entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<CurrencyExchange, String> {
        let from_currency = self.from_currency.ok_or_else(|| "from_currency is required".to_string())?;
        let to_currency = self.to_currency.ok_or_else(|| "to_currency is required".to_string())?;
        let rate = self.rate.ok_or_else(|| "rate is required".to_string())?;
        let effective_from = self.effective_from.ok_or_else(|| "effective_from is required".to_string())?;

        Ok(CurrencyExchange {
            id: Uuid::new_v4(),
            company_id: self.company_id,
            from_currency,
            to_currency,
            rate,
            effective_from,
            effective_to: self.effective_to,
            metadata: AuditMetadata::default(),
        })
    }
}
