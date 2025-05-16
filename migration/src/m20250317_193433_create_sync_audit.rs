use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SyncAudit::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncAudit::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SyncAudit::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SyncAudit::CompletedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(SyncAudit::SegmentSize).integer().not_null())
                    .col(ColumnDef::new(SyncAudit::Status).integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SyncAuditSegment::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncAuditSegment::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SyncAuditSegment::SyncAuditId)
                            .integer()
                            .auto_increment()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SyncAuditSegment::StartBlock)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SyncAuditSegment::EndBlock)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SyncAuditSegment::NumBlocks)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SyncAuditSegment::MinResponseMs).integer())
                    .col(ColumnDef::new(SyncAuditSegment::MaxResponseMs).integer())
                    .col(ColumnDef::new(SyncAuditSegment::MeanResponseMs).integer())
                    .col(ColumnDef::new(SyncAuditSegment::MedianResponseMs).integer())
                    .col(ColumnDef::new(SyncAuditSegment::P99ResponseMs).integer())
                    .col(ColumnDef::new(SyncAuditSegment::TotalDurationMs).integer())
                    .col(
                        ColumnDef::new(SyncAuditSegment::NumErrors)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(SyncAuditSegment::Status).integer())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-sync_audit_segment-audit_id")
                            .from(SyncAuditSegment::Table, SyncAuditSegment::SyncAuditId)
                            .to(SyncAudit::Table, SyncAudit::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SyncAuditError::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SyncAuditError::Id)
                            .integer()
                            .auto_increment()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SyncAuditError::SyncAuditSegmentId)
                            .integer()
                            .auto_increment()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SyncAuditError::BlockNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SyncAuditError::ErrorType).string())
                    .col(ColumnDef::new(SyncAuditError::ErrorMessage).text())
                    .col(
                        ColumnDef::new(SyncAuditError::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-sync_audit_error-record_id")
                            .from(SyncAuditError::Table, SyncAuditError::SyncAuditSegmentId)
                            .to(SyncAuditSegment::Table, SyncAuditSegment::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SyncAuditError::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SyncAuditSegment::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SyncAudit::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum SyncAudit {
    Table,
    Id,
    StartedAt,
    CompletedAt,
    SegmentSize,
    Status,
}

#[derive(Iden)]
enum SyncAuditSegment {
    Table,
    Id,
    SyncAuditId,
    StartBlock,
    EndBlock,
    NumBlocks,
    MinResponseMs,
    MaxResponseMs,
    MeanResponseMs,
    MedianResponseMs,
    P99ResponseMs,
    TotalDurationMs,
    NumErrors,
    Status,
}

#[derive(Iden)]
enum SyncAuditError {
    Table,
    Id,
    SyncAuditSegmentId,
    BlockNumber,
    ErrorType,
    ErrorMessage,
    CreatedAt,
}
