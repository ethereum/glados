use sea_orm_migration::{prelude::*, schema::*};

const FK_CONTENT_ID: &str = "FK-audit_latest-content_id";
const FK_AUDIT_ID: &str = "FK-audit_latest-audit_id";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuditLatest::Table)
                    .if_not_exists()
                    .col(integer(AuditLatest::ContentId).primary_key())
                    .col(integer(AuditLatest::AuditId))
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_CONTENT_ID)
                            .from(AuditLatest::Table, AuditLatest::ContentId)
                            .to(Content::Table, Content::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name(FK_AUDIT_ID)
                            .from(AuditLatest::Table, AuditLatest::ContentId)
                            .to(Audit::Table, Audit::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuditLatest::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AuditLatest {
    Table,
    ContentId,
    AuditId,
}

#[derive(DeriveIden)]
enum Content {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Audit {
    Table,
    Id,
}
