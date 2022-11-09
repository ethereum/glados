#[cfg(test)]
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DbConn, DbErr, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, Set,
};

use migration::{Migrator, MigratorTrait};

use crate::node;

async fn setup_database() -> Result<DbConn, DbErr> {
    let base_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_owned());

    let db: DbConn = Database::connect(&base_url).await?;

    Migrator::up(&db, None).await.unwrap();

    println!("Setup database schema");

    return Ok(db);
}

#[tokio::test]
async fn test_node_crud() -> Result<(), DbErr> {
    let db = setup_database().await?;

    let node_id_a: Vec<u8> = vec![
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];
    let node_id_b: Vec<u8> = vec![
        31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9,
        8, 7, 6, 5, 4, 3, 2, 1, 0,
    ];

    let node_a = node::ActiveModel {
        id: NotSet,
        node_id: Set(node_id_a.clone()),
    };
    let node_b = node::ActiveModel {
        id: NotSet,
        node_id: Set(node_id_b.clone()),
    };

    assert_eq!(node::Entity::find().count(&db).await?, 0);

    let node_a = node_a.insert(&db).await?;
    println!("Inserted: node_a={:?}", node_a);

    let node_b = node_b.insert(&db).await?;
    println!("Inserted: node_b={:?}", node_b);

    assert_eq!(node::Entity::find().count(&db).await?, 2);

    let node_a = node::Entity::find()
        .filter(node::Column::NodeId.eq(node_id_a.clone()))
        .one(&db)
        .await?
        .unwrap();

    assert_eq!(node_a.node_id, node_id_a);

    let node_b = node::Entity::find()
        .filter(node::Column::NodeId.eq(node_id_b.clone()))
        .one(&db)
        .await?
        .unwrap();

    assert_eq!(node_b.node_id, node_id_b);

    Ok(())
}

#[tokio::test]
async fn crud_record() -> Result<(), DbErr> {
    let db = setup_database().await?;

    use enr::{k256, EnrBuilder};
    use std::net::Ipv4Addr;

    let raw_signing_key: Vec<u8> = vec![
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];

    // generate a random secp256k1 key
    let key = k256::ecdsa::SigningKey::from_bytes(&raw_signing_key).unwrap();

    let ip = Ipv4Addr::new(192, 168, 0, 1);
    let enr = EnrBuilder::new("v4")
        .ip4(ip)
        .tcp4(8000)
        .build(&key)
        .unwrap();

    assert_eq!(enr.ip4(), Some("192.168.0.1".parse().unwrap()));
    assert_eq!(enr.id(), Some("v4".into()));

    Ok(())
}
