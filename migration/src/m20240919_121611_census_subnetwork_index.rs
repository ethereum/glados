use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX_CENSUS_SUBNET_INDEX: &str = "idx_census_subnet";
const INDEX_CENSUS_NODE_SUBNET_INDEX: &str = "idx_census_node_subnet";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let _ = manager
            .create_index(
                Index::create()
                    .name(INDEX_CENSUS_SUBNET_INDEX)
                    .table(Census::Table)
                    .col(Census::SubNetwork)
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
            .drop_index(Index::drop().name(INDEX_CENSUS_SUBNET_INDEX).to_owned())
            .await;
        manager
            .drop_index(
                Index::drop()
                    .name(INDEX_CENSUS_NODE_SUBNET_INDEX)
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
