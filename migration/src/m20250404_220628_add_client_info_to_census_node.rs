use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CensusNode::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::ClientName).string().null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::ClientVersion).string().null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::ShortCommit).string().null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::OperatingSystem).string().null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::CpuArchitecture).string().null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(CensusNode::ProgrammingLanguageVersion)
                            .string()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CensusNode::Table)
                    .drop_column(CensusNode::ClientName)
                    .drop_column(CensusNode::ClientVersion)
                    .drop_column(CensusNode::ShortCommit)
                    .drop_column(CensusNode::OperatingSystem)
                    .drop_column(CensusNode::CpuArchitecture)
                    .drop_column(CensusNode::ProgrammingLanguageVersion)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum CensusNode {
    Table,
    ClientName,
    ClientVersion,
    ShortCommit,
    OperatingSystem,
    CpuArchitecture,
    ProgrammingLanguageVersion,
}
