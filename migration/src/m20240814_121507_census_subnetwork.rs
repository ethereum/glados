use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX_CENSUS_SUBNET_INDEX: &str = "idx_census_subnet";
const INDEX_CENSUS_NODE_SUBNET_INDEX: &str = "idx_census_node_subnet";

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
        let _ = manager
            .create_index(
                Index::create()
                    .name(INDEX_CENSUS_SUBNET_INDEX)
                    .table(Census::Table)
                    .col(Census::SubNetwork)
                    .to_owned(),
            )
            .await;
        let _ = manager
            .alter_table(
                Table::alter()
                    .table(CensusNode::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::SubNetwork).unsigned().default(0),
                    )
                    .to_owned(),
            )
            .await;
        manager
            .create_index(
                Index::create()
                    .name(INDEX_CENSUS_NODE_SUBNET_INDEX)
                    .table(CensusNode::Table)
                    .col(CensusNode::SubNetwork)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .alter_table(
                Table::alter()
                    .table(Census::Table)
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
