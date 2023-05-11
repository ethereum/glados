use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ClientInfo::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClientInfo::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx_client_info-id")
                            .col(ClientInfo::Id),
                    )
                    .col(ColumnDef::new(ClientInfo::VersionInfo).string().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ClientInfo::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ClientInfo {
    Table,
    Id,
    VersionInfo,
}
