use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX_AUDIT_STATS_PERF: &str = "idx_audit_stats_perf";
const INDEX_CONTENT_PROTOCOL_ID_IDX: &str = "idx_content_protocol_id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .create_index(
                Index::create()
                    .name(INDEX_AUDIT_STATS_PERF)
                    .table(ContentAudit::Table)
                    .col(ContentAudit::Result)
                    .col(ContentAudit::CreatedAt)
                    .col(ContentAudit::ContentKey)
                    .to_owned(),
            )
            .await;

        manager
            .create_index(
                Index::create()
                    .name(INDEX_CONTENT_PROTOCOL_ID_IDX)
                    .table(Content::Table)
                    .col(Content::ProtocolId)
                    .col(Content::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .drop_index(Index::drop().name(INDEX_AUDIT_STATS_PERF).to_owned())
            .await;
        manager
            .drop_index(Index::drop().name(INDEX_CONTENT_PROTOCOL_ID_IDX).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ContentAudit {
    Table,
    ContentKey,
    CreatedAt,
    Result,
}

#[derive(Iden)]
enum Content {
    Table,
    ProtocolId,
    Id,
}
