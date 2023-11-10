//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.7
use crate::content;
use crate::utils;
use anyhow::anyhow;
use anyhow::{bail, Result};
use chrono::DateTime;
use chrono::Utc;
use clap::ValueEnum;
use ethportal_api::OverlayContentKey;
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Copy, Debug, Clone, Eq, PartialEq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum AuditResult {
    Failure = 0,
    Success = 1,
    Pending = 2,
    Errored = 3,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq, EnumIter, DeriveActiveEnum, ValueEnum)]
#[clap(rename_all = "snake_case")]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
/// Each strategy is responsible for selecting which content key(s) to begin audits for.
pub enum SelectionStrategy {
    /// Content that is:
    /// 1. Not yet audited
    /// 2. Sorted by date entered into glados database (newest first).
    Latest = 0,
    /// Randomly selected content.
    Random = 1,
    /// Content that looks for failed audits and checks whether the data is still missing.
    /// 1. Key was audited previously
    /// 2. Latest audit for the key failed (data absent)
    /// 3. Keys sorted by date audited (keys with oldest failed audit first)
    Failed = 2,
    /// Content that is:
    /// 1. Not yet audited.
    /// 2. Sorted by date entered into glados database (oldest first).
    SelectOldestUnaudited = 3,
    /// Perform a single audit for a previously audited content key.
    SpecificContentKey = 4,
}

impl AuditResult {
    pub fn as_text(&self) -> String {
        match self {
            AuditResult::Failure => "Fail".to_string(),
            AuditResult::Success => "Success".to_string(),
            AuditResult::Pending => "Pending".to_string(),
            AuditResult::Errored => "Error".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "content_audit")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub content_key: i32,
    pub client_info: Option<i32>,
    pub node: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub strategy_used: Option<SelectionStrategy>,
    pub result: AuditResult,
    pub trace: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content::Entity",
        from = "Column::ContentKey",
        to = "super::content::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Content,
    #[sea_orm(
        belongs_to = "super::client_info::Entity",
        from = "Column::ClientInfo",
        to = "super::client_info::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ClientInfo,
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::Node",
        to = "super::node::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Node,
}

impl Related<super::content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Content.def()
    }
}

impl Related<super::client_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ClientInfo.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Create a new, pending audit entry.
pub async fn create(
    content_key_model_id: i32,
    client_info_id: i32,
    node_id: i32,
    strategy_used: SelectionStrategy,
    conn: &DatabaseConnection,
) -> Result<Model> {
    let content_audit = ActiveModel {
        id: NotSet,
        content_key: Set(content_key_model_id),
        client_info: Set(Some(client_info_id)),
        node: Set(Some(node_id)),
        created_at: Set(Utc::now()),
        result: Set(AuditResult::Pending),
        strategy_used: Set(Some(strategy_used)),
        trace: Set("".to_owned()),
    };
    Ok(content_audit.insert(conn).await?)
}

/// Retrieve the audit, update it from pending to success, failure, or errored.
pub async fn record_result(
    audit_id: i32,
    audit_result: AuditResult,
    trace_string: String,
    conn: &DatabaseConnection,
) -> Result<Model> {
    let audit: Option<Model> = Entity::find_by_id(audit_id).one(conn).await?;
    let audit = audit.ok_or(anyhow!("No audit found for id {}", audit_id))?;
    let mut audit: ActiveModel = audit.into();
    audit.result = Set(audit_result);
    audit.trace = Set(trace_string);
    Ok(audit.update(conn).await?)
}

pub async fn get_audits<T: OverlayContentKey>(
    content_key: &T,
    conn: &DatabaseConnection,
) -> Result<Vec<Model>> {
    let Some(content_key_model) = content::get(content_key, conn).await?
    else {
    bail!("Expected stored content_key found none.")
    };
    Ok(Entity::find()
        .filter(Column::ContentKey.eq(content_key_model.id))
        .all(conn)
        .await?)
}

impl SelectionStrategy {
    /// This performs the function of Display, which is not able to be implemented
    /// for this enum.
    ///
    /// SelectionStrategy derive macro DeriveActiveEnum introduces a conflicting
    /// Display implementation.
    pub fn as_text(&self) -> String {
        match self {
            SelectionStrategy::Latest => "Latest".to_string(),
            SelectionStrategy::Random => "Random".to_string(),
            SelectionStrategy::Failed => "Failed".to_string(),
            SelectionStrategy::SelectOldestUnaudited => "Select Oldest Unaudited".to_string(),
            SelectionStrategy::SpecificContentKey => "Specific Content Key".to_string(),
        }
    }
}

impl Model {
    pub fn is_success(&self) -> bool {
        self.result == AuditResult::Success
    }
    pub fn is_pending(&self) -> bool {
        self.result == AuditResult::Pending
    }
    pub fn created_at_local_time(&self) -> String {
        self.created_at.with_timezone(&chrono::Local).to_rfc2822()
    }
    pub fn created_at_humanized(&self) -> String {
        utils::time_ago(self.created_at, Utc::now())
    }
    /// A convenience method for displaying the strategy.
    ///
    /// A few early databse entries do not have a recorded strategy.
    pub fn strategy_as_text(&self) -> String {
        match &self.strategy_used {
            Some(s) => s.as_text(),
            None => "No strategy recorded".to_string(),
        }
    }
}
