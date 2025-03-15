use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        manager
            .create_table(
                Table::create()
                    .table(AuditResultLatest::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditResultLatest::ContentId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AuditResultLatest::LastAudited)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditResultLatest::Result)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuditResultLatest::StrategyUsed).integer())
                    .col(
                        ColumnDef::new(AuditResultLatest::FirstAvailableAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditResultLatest::ContentType)
                            .unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditResultLatest::ProtocolId)
                            .unsigned()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuditResultLatest::ContentKey)
                            .binary()
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .name("idx-content-type-and-first-available-at")
                            .col(AuditResultLatest::ContentType)
                            .col(AuditResultLatest::FirstAvailableAt),
                    )
                    .to_owned(),
            )
            .await?;

        db.execute_unprepared(
            "INSERT INTO audit_result_latest
            SELECT
                audits.content_id,
                audits.last_audited,
                audits.result,
                audits.strategy_used,
                content.first_available_at,
                get_byte(content.content_key, 0) AS content_type,
                content.protocol_id,
                content.content_key
            FROM (
                SELECT DISTINCT ON (content_key)
                    content_key AS content_id,
                    created_at AS last_audited,
                    result,
                    strategy_used
                FROM content_audit
                WHERE strategy_used = 5
                ORDER BY
                  content_key,
                  created_at DESC
            ) audits
            JOIN content ON audits.content_id = content.id
            WHERE content.protocol_id = 0
            ;",
        )
        .await?;

        db.execute_unprepared(
            "CREATE OR REPLACE FUNCTION public.update_audit_result_latest()
                RETURNS trigger
                LANGUAGE plpgsql
            AS $function$
            BEGIN
                INSERT INTO audit_result_latest
                SELECT
                    audits.content_id,
                    audits.last_audited,
                    audits.result,
                    audits.strategy_used,
                    content.first_available_at,
                    get_byte(content.content_key, 0) AS content_type,
                    content.protocol_id,
                    content.content_key
                FROM (
                    SELECT DISTINCT ON (content_key)
                        content_key AS content_id,
                        created_at AS last_audited,
                        result,
                        strategy_used
                    FROM content_audit
                    WHERE strategy_used = 5
                    ORDER BY
                      content_key,
                      created_at DESC
                ) audits
                JOIN content ON audits.content_id = content.id
                WHERE content.protocol_id = 0
                ON CONFLICT (content_id) DO
                UPDATE
                    SET
                        content_id = EXCLUDED.content_id,
                        last_audited = EXCLUDED.last_audited,
                        result = EXCLUDED.result,
                        strategy_used = EXCLUDED.strategy_used,
                        first_available_at = EXCLUDED.first_available_at,
                        content_type = EXCLUDED.content_type,
                        protocol_id = EXCLUDED.protocol_id,
                        content_key = EXCLUDED.content_key
                    ;
                RETURN NEW;
            END;
            $function$
            ;",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER audit_result_latest_trigger
            AFTER INSERT ON public.content_audit
            FOR EACH ROW EXECUTE FUNCTION update_audit_result_latest()",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TRIGGER audit_result_latest_trigger ON public.content_audit;")
            .await?;

        db.execute_unprepared("DROP FUNCTION public.update_audit_result_latest();")
            .await?;

        manager
            .drop_table(Table::drop().table(AuditResultLatest::Table).to_owned())
            .await?;

        Ok(())
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum AuditResultLatest {
    Table,
    ContentId,
    LastAudited,
    Result,
    StrategyUsed,
    FirstAvailableAt,
    ContentType,
    ProtocolId,
    ContentKey,
}
