use anyhow::Result;
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "query_response_node")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub query_response_id: i32,
    pub node_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::query_response::Entity",
        from = "Column::QueryResponseId",
        to = "super::query_response::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    QueryResponse,
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::NodeId",
        to = "super::node::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Node,
}

impl Related<super::query_response::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::QueryResponse.def()
    }
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Create a new query response node entity.
pub async fn create(
    node_id: i32,
    query_response_ids: Vec<i32>,
    conn: &DatabaseConnection,
) -> Result<()> {
    let models = query_response_ids
        .into_iter()
        .map(|query_response_id| ActiveModel {
            id: NotSet,
            query_response_id: Set(query_response_id),
            node_id: Set(node_id),
        })
        .collect::<Vec<_>>();

    Entity::insert_many(models).exec(conn).await?;
    Ok(())
}
