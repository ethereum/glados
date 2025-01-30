use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        todo!();

        manager
            .create_table(
                Table::create()
                    .table(AuditInternalFailure::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuditInternalFailure::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AuditInternalFailure::Audit)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_auditinternalfailure_audit")
                            .from(AuditInternalFailure::Table, AuditInternalFailure::Audit)
                            .to(Audit::Table, Audit::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(AuditInternalFailure::SenderClientInfo)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_auditinternalfailure_sender_client_info")
                            .from(
                                AuditInternalFailure::Table,
                                AuditInternalFailure::SenderClientInfo,
                            )
                            .to(ClientInfo::Table, ClientInfo::Id),
                    )
                    .col(
                        ColumnDef::new(AuditInternalFailure::SenderNode)
                            .integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("FK_auditinternalfailure_sender_node")
                            .from(AuditInternalFailure::Table, AuditInternalFailure::SenderNode)
                            .to(Node::Table, Node::Id),
                    )
                    .col(
                        ColumnDef::new(AuditInternalFailure::FailureType)
                            .integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        todo!();

        manager
            .drop_table(Table::drop().table(Post::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum AuditInternalFailure {
    Table,
    Id,
    // Foreign key
    Audit,
    // Foreign key
    SenderClientInfo,
    // Foreign key
    SenderNode,
    // Custom enum
    FailureType,
}

#[derive(Iden)]
enum Audit {
    Table,
    Id,
}

#[derive(Iden)]
enum ClientInfo {
    Table,
    Id,
}

#[derive(Iden)]
enum Content {
    Table,
    Id,
}
