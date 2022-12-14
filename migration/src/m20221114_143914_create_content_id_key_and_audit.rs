use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ContentId
        manager
            .create_table(
                Table::create()
                    .table(ContentId::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentId::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContentId::ContentId)
                            .binary_len(32)
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-contentid-content_id")
                            .col(ContentId::ContentId),
                    )
                    .to_owned(),
            )
            .await?;

        // ContentKey
        manager
            .create_table(
                Table::create()
                    .table(ContentKey::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentKey::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ContentKey::ContentId).integer().not_null())
                    //.index(Index::create().name("idx-contentkey-content_id").col(ContentKey::ContentId))
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_conent_key_content_id")
                            .from(ContentKey::Table, ContentKey::ContentId)
                            .to(ContentId::Table, ContentId::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(ContentKey::ContentKey).binary().not_null())
                    //.index(Index::create().unique().name("idx-contentkey-content_key").col(ContentKey::ContentKey))
                    .col(
                        ColumnDef::new(ContentKey::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // ContentAudit
        manager
            .create_table(
                Table::create()
                    .table(ContentAudit::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContentAudit::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContentAudit::ContentKey)
                            .integer()
                            .not_null(),
                    )
                    //.index(Index::create().name("idx-contentaudit-content_key").col(ContentAudit::ContentKey))
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_conentaudit_content_key")
                            .from(ContentAudit::Table, ContentAudit::ContentKey)
                            .to(ContentKey::Table, ContentKey::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(ContentAudit::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    //.index(Index::create().name("idx-contentaudit-created_at").col(ContentAudit::CreatedAt))
                    .col(ColumnDef::new(ContentAudit::Result).integer().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContentId::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ContentKey::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ContentAudit::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum ContentId {
    Table,
    Id,
    ContentId,
}

#[derive(Iden)]
enum ContentKey {
    Table,
    Id,
    ContentId,
    ContentKey,
    CreatedAt,
}

#[derive(Iden)]
enum ContentAudit {
    Table,
    Id,
    ContentKey,
    CreatedAt,
    Result,
}
