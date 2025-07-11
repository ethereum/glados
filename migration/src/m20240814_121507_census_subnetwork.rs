use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .alter_table(
                Table::alter()
                    .table(Census::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Census::SubNetwork).unsigned().default(0),
                    )
                    .to_owned(),
            )
            .await;

        manager
            .alter_table(
                Table::alter()
                    .table(CensusNode::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::SubNetwork).unsigned().default(0),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .alter_table(
                Table::alter()
                    .table(CensusNode::Table)
                    .drop_column(Census::SubNetwork)
                    .to_owned(),
            )
            .await;
        manager
            .alter_table(
                Table::alter()
                    .table(Census::Table)
                    .drop_column(Census::SubNetwork)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Census {
    Table,
    SubNetwork,
}

#[derive(Iden)]
enum CensusNode {
    Table,
    SubNetwork,
}
