use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEX_CENSUS_RECORD_ID_SURVEYED_AT: &str = "idx_census_record_id_surveyed_at";

#[derive(Iden)]
enum CensusNode {
    Table,
    RecordId,
    SurveyedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name(INDEX_CENSUS_RECORD_ID_SURVEYED_AT)
                    .table(CensusNode::Table)
                    .col(CensusNode::RecordId)
                    .col(CensusNode::SurveyedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(INDEX_CENSUS_RECORD_ID_SURVEYED_AT)
                    .to_owned(),
            )
            .await
    }
}
