use sea_orm_migration::{prelude::*, schema::*};

const FK_CENSUS_ID: &str = "FK-census_node-census_id";
const FK_NODE_ENR_ID: &str = "FK-census_node-node_enr_id";

const IDX_CENSUS_ID_NODE_ENR_ID: &str = "IDX-census_node-census_id-node_enr_id";
const IDX_NODE_ENR_ID_SURVEYED_AT: &str = "IDX-census_node-node_enr_id-surveyed_at";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CensusNode::Table)
                    .if_not_exists()
                    .col(pk_auto(CensusNode::Id))
                    .col(integer(CensusNode::CensusId))
                    .col(integer(CensusNode::NodeEnrId))
                    .col(timestamp_with_time_zone(CensusNode::SurveyedAt))
                    .col(binary_len(CensusNode::DataRadius, 32))
                    .col(big_unsigned(CensusNode::DataRadiusHigh))
                    .col(text_null(CensusNode::ClientName))
                    .col(text_null(CensusNode::ClientVersion))
                    .col(text_null(CensusNode::ShortCommit))
                    .col(text_null(CensusNode::OperatingSystem))
                    .col(text_null(CensusNode::CpuArchitecture))
                    .col(text_null(CensusNode::ProgrammingLanguageVersion))
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_CENSUS_ID)
                            .from(CensusNode::Table, CensusNode::CensusId)
                            .to(Census::Table, Census::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_NODE_ENR_ID)
                            .from(CensusNode::Table, CensusNode::NodeEnrId)
                            .to(NodeEnr::Table, NodeEnr::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(CensusNode::Table)
                    .name(IDX_CENSUS_ID_NODE_ENR_ID)
                    .col(CensusNode::CensusId)
                    .col(CensusNode::NodeEnrId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(CensusNode::Table)
                    .name(IDX_NODE_ENR_ID_SURVEYED_AT)
                    .col(CensusNode::NodeEnrId)
                    .col(CensusNode::SurveyedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CensusNode::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum CensusNode {
    Table,
    Id,
    CensusId,
    NodeEnrId,
    SurveyedAt,
    DataRadius,
    DataRadiusHigh,
    ClientName,
    ClientVersion,
    ShortCommit,
    OperatingSystem,
    CpuArchitecture,
    ProgrammingLanguageVersion,
}

#[derive(DeriveIden)]
enum Census {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum NodeEnr {
    Table,
    Id,
}
