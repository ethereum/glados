use ethportal_api::HistoryContentKey;
use sea_orm::prelude::*;
use serde::Deserialize;
use strum::{EnumMessage, EnumString};

// Not using the constants in ethportal-api because seaorm does not support DeriveActiveEnum from a
// variable
#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    EnumIter,
    DeriveActiveEnum,
    Deserialize,
    EnumMessage,
    EnumString,
)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
#[strum(serialize_all = "snake_case")]
pub enum ContentType {
    #[strum(message = "Block bodies")]
    BlockBodies = 0,
    #[strum(message = "Block receipts")]
    BlockReceipts = 1,
}

impl AsRef<ContentType> for HistoryContentKey {
    fn as_ref(&self) -> &ContentType {
        match self {
            HistoryContentKey::BlockBody(_) => &ContentType::BlockBodies,
            HistoryContentKey::BlockReceipts(_) => &ContentType::BlockReceipts,
            _ => &ContentType::BlockBodies,
        }
    }
}
