use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

const IDX_CONTENT_ID: &str = "IDX-content-content_id";
const IDX_CONTENT_KEY: &str = "IDX-content-content_key";
const IDX_SUBPROTOCOL_CONTENT_KEY: &str = "IDX-content-subprotocol-content_key";
const IDX_SUBPROTOCOL_BLOCK_NUMBER_CONTENT_TYPE: &str =
    "IDX-content-subprotocol-block_number-content_type-id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Content::Table)
                    .if_not_exists()
                    .col(pk_auto(Content::Id))
                    .col(unsigned(Content::Subprotocol))
                    .col(binary_len(Content::ContentId, 32))
                    .col(blob(Content::ContentKey))
                    .col(unsigned(Content::ContentType))
                    .col(big_integer_null(Content::BlockNumber))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Content::Table)
                    .name(IDX_CONTENT_ID)
                    .col(Content::ContentId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .table(Content::Table)
                    .name(IDX_CONTENT_KEY)
                    .col(Content::ContentKey)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .unique()
                    .table(Content::Table)
                    .name(IDX_SUBPROTOCOL_CONTENT_KEY)
                    .col(Content::Subprotocol)
                    .col(Content::ContentKey)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .unique()
                    .table(Content::Table)
                    .name(IDX_SUBPROTOCOL_BLOCK_NUMBER_CONTENT_TYPE)
                    .col(Content::Subprotocol)
                    .col(Content::BlockNumber)
                    .col(Content::ContentType)
                    .col(Content::Id)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Content::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
#[allow(clippy::enum_variant_names)]
enum Content {
    Table,
    Id,
    Subprotocol,
    ContentId,
    ContentKey,
    ContentType,
    BlockNumber,
}
