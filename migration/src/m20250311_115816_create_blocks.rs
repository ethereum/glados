use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Block::Table)
                    .col(
                        ColumnDef::new(Block::Hash)
                            .binary()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Block::Number).integer().not_null())
                    .col(
                        ColumnDef::new(Block::Timestamp)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-block-timestamp")
                            .col(Block::Timestamp),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Block::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Block {
    Table,
    Hash,
    Number,
    Timestamp,
}
