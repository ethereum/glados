use std::time::UNIX_EPOCH;

use super::query_response;
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use ethportal_api::types::query_trace::QueryTrace;
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "query_trace")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub timestamp: DateTime<Utc>,
    pub origin_record_id: i32,
    pub content_successfully_received_from: Option<i32>,
    pub target_content: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::query_response::Entity")]
    QueryResponse,
}

impl Related<super::query_response::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::QueryResponse.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub async fn create(query_trace: QueryTrace, conn: &DatabaseConnection) -> Result<Model> {
    // Get query timestamp.
    let timestamp = query_trace.started_at_ms;
    let timestamp: DateTime<Utc> = {
        let duration = timestamp.duration_since(UNIX_EPOCH).unwrap();
        Utc.timestamp_opt(duration.as_secs() as i64, duration.as_nanos() as u32)
            .single()
            .unwrap()
    };

    // Get origin record.
    let origin_node_id = query_trace.origin;
    let origin_record = match super::record::Entity::find()
        .filter(super::record::Column::NodeId.eq(origin_node_id.raw().to_vec()))
        .one(conn)
        .await?
    {
        Some(record) => record,
        None => {
            return Err(anyhow::anyhow!(
                "No record found for node ID {}",
                origin_node_id
            ));
        }
    };

    // Get the most recent record for the node that responded (if any).
    let received_from_node_id = query_trace.received_from;
    let received_from_record = if let Some(node_id) = received_from_node_id {
        super::record::Entity::find()
            .filter(super::record::Column::NodeId.eq(node_id.raw().to_vec()))
            .one(conn)
            .await?
    } else {
        None
    };

    // Get the target content's db ID.
    let content_id_bytes = query_trace.target_id.to_vec();
    let target_content = match super::content::Entity::find()
        .filter(super::content::Column::ContentId.eq(content_id_bytes.clone()))
        .one(conn)
        .await?
    {
        Some(content) => content.id,
        None => {
            return Err(anyhow::anyhow!(
                "No content found for content ID {:?}",
                content_id_bytes
            ));
        }
    };

    // Save query_trace model into the database.
    let query_trace_model = ActiveModel {
        id: NotSet,
        timestamp: Set(timestamp),
        origin_record_id: Set(origin_record.id),
        content_successfully_received_from: Set(received_from_record.map(|r| r.id)),
        target_content: Set(target_content),
    };

    let query_trace_model = match query_trace_model.insert(conn).await {
        Ok(query_trace) => query_trace,
        Err(err) => {
            return Err(anyhow::anyhow!("Failed to save query trace: {:?}", err));
        }
    };

    // Create all of the QueryResponse entries for this QueryTrace.
    query_response::create(query_trace_model.id, query_trace.responses, conn).await?;

    // Create QueryResponse objects for each response record, tied to the QueryTrace.
    Ok(query_trace_model)
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test::setup_database;

    #[tokio::test]
    async fn test_create_query_trace() -> Result<(), DbErr> {
        let conn = setup_database().await?;

        // 1.) create a set of ENRs/Node IDs to use as origin and response records.

        // 2.) create a query trace with the origin and response records and a content ID.
        // 3.) create a new query trace in the DB.
        // 4.) read it and verify that it was created succesfully.

        // let query_trace = QueryTrace {
        //     started_at_ms: Utc::now(),
        //     origin: 1,
        //     received_from: Some(2),
        //     target_id: [1, 2, 3, 4, 5],
        // };

        // let query_trace = create(query_trace, &conn.0).await.unwrap();

        // assert_eq!(query_trace.id, 1);
        // assert_eq!(query_trace.timestamp, Utc::now());
        // assert_eq!(query_trace.origin_record_id, 1);
        // assert_eq!(query_trace.content_successfully_received_from, Some(2));
        // assert_eq!(query_trace.target_content, 1);

        Ok(())
    }
}
