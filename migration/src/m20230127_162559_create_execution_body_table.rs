use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Execution body table
        manager
            .create_table(
                Table::create()
                    .table(ExecutionBody::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExecutionBody::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ExecutionBody::ContentKey).integer())
                    .col(
                        ColumnDef::new(ExecutionBody::BlockNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExecutionBody::BlockHash)
                            .binary_len(32)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_execution_body_content_key")
                            .from(ExecutionBody::Table, ExecutionBody::ContentKey)
                            .to(ContentKey::Table, ContentKey::Id)
                            .on_delete(ForeignKeyAction::SetNull)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_body-content_key")
                            .col(ExecutionBody::ContentKey),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_body-block_number")
                            .col(ExecutionBody::BlockNumber),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-execution_body-block_hash")
                            .col(ExecutionBody::BlockHash),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ExecutionBody::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum ContentKey {
    Table,
    Id,
}

#[derive(Iden)]
enum ExecutionBody {
    Table,
    Id,
    ContentKey,
    BlockNumber,
    BlockHash,
}
