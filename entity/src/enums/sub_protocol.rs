use clap::ValueEnum;
use sea_orm::prelude::*;
use serde::Deserialize;

/// Portal network sub-protocol
#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumIter, DeriveActiveEnum, Deserialize, ValueEnum)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum SubProtocol {
    History = 0,
}

impl SubProtocol {
    pub fn as_text(&self) -> String {
        match self {
            SubProtocol::History => "History".to_string(),
        }
    }
}
