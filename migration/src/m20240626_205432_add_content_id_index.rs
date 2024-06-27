use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX_CONTENT_ID: &str = "idx_content_content_id_fk";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name(INDEX_CONTENT_ID)
                    .table(Content::Table)
                    .col(Content::ContentId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name(INDEX_CONTENT_ID).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Content {
    Table,
    ContentId,
}
