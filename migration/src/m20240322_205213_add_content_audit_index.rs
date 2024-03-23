use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX_CONTENT_AUDIT_CONTENT_RELATION: &str = "idx_content_audit_content_fk";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name(INDEX_CONTENT_AUDIT_CONTENT_RELATION)
                    .table(ContentAudit::Table)
                    .col(ContentAudit::ContentKey)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(INDEX_CONTENT_AUDIT_CONTENT_RELATION)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ContentAudit {
    Table,
    ContentKey,
}
