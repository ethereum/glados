use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Census::Table)
                    .col(
                        ColumnDef::new(Census::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Census::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Census::Duration).integer().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx-census-started-at")
                    .table(Census::Table)
                    .col(Census::StartedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(CensusNode::Table)
                    .col(
                        ColumnDef::new(CensusNode::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CensusNode::CensusId).integer().not_null())
                    .col(ColumnDef::new(CensusNode::RecordId).integer().not_null())
                    .col(
                        ColumnDef::new(CensusNode::SurveyedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CensusNode::DataRadius)
                            .binary_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CensusNode::DataRadiusHigh)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-census_node-census_id")
                            .from(CensusNode::Table, CensusNode::CensusId)
                            .to(Census::Table, Census::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-census_node-record_id")
                            .from(CensusNode::Table, CensusNode::RecordId)
                            .to(Record::Table, Record::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-uniqui-census-started_at")
                            .table(Census::Table)
                            .col(CensusNode::CensusId)
                            .col(CensusNode::RecordId),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Census::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CensusNode::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Census {
    Table,
    Id,
    StartedAt,
    Duration,
}

#[derive(Iden)]
enum CensusNode {
    Table,
    Id,
    CensusId,
    RecordId,
    SurveyedAt,
    DataRadius,
    DataRadiusHigh,
}

#[derive(Iden)]
enum Record {
    Table,
    Id,
}
