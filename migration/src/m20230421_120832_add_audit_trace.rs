use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ContentAudit::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ContentAudit::Trace).string().default(""),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContentAudit::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ContentAudit {
    Table,
    Trace,
}
