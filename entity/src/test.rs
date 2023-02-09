#![allow(unused_imports)]
#[cfg(test)]
use chrono::prelude::*;
use ethereum_types::H256;
use ethportal_api::types::content_key::{BlockHeaderKey, HistoryContentKey};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DbConn, DbErr, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, Set,
};

use migration::{Migrator, MigratorTrait};

use crate::{contentaudit, contentkey, node};

#[allow(dead_code)]
async fn setup_database() -> Result<DbConn, DbErr> {
    let base_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_owned());

    let conn: DbConn = Database::connect(&base_url).await?;

    Migrator::up(&conn, None).await.unwrap();

    println!("Setup database schema");

    Ok(conn)
}

#[tokio::test]
async fn test_node_crud() -> Result<(), DbErr> {
    let conn = setup_database().await?;

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

    assert_eq!(node::Entity::find().count(&conn).await?, 0);

    let node_a = node_a.insert(&conn).await?;
    println!("Inserted: node_a={:?}", node_a);

    let node_b = node_b.insert(&conn).await?;
    println!("Inserted: node_b={:?}", node_b);

    assert_eq!(node::Entity::find().count(&conn).await?, 2);

    let node_a = node::Entity::find()
        .filter(node::Column::NodeId.eq(node_id_a.clone()))
        .one(&conn)
        .await?
        .unwrap();

    assert_eq!(node_a.node_id, node_id_a);

    let node_b = node::Entity::find()
        .filter(node::Column::NodeId.eq(node_id_b.clone()))
        .one(&conn)
        .await?
        .unwrap();

    assert_eq!(node_b.node_id, node_id_b);

    Ok(())
}

#[tokio::test]
async fn crud_record() -> Result<(), DbErr> {
    let _conn = setup_database().await?;

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

#[tokio::test]
async fn test_contentkey_get_or_create() -> Result<(), DbErr> {
    let conn = setup_database().await?;

    let block_hash = H256::from_slice(
        &hex::decode("d1c390624d3bd4e409a61a858e5dcc5517729a9170d014a6c96530d64dd8621d").unwrap(),
    );

    let header_content_key = HistoryContentKey::BlockHeader(BlockHeaderKey {
        block_hash: block_hash.to_fixed_bytes(),
    });

    // Ensure our database is empty
    assert_eq!(contentkey::Entity::find().count(&conn).await?, 0);

    let content_key_a = contentkey::get_or_create(&header_content_key, &conn)
        .await
        .unwrap();

    // Ensure our database now has an entry
    assert_eq!(contentkey::Entity::find().count(&conn).await?, 1);

    let content_key_b = contentkey::get_or_create(&header_content_key, &conn)
        .await
        .unwrap();

    // Ensure the existing entry was found.
    assert_eq!(contentkey::Entity::find().count(&conn).await?, 1);
    assert_eq!(content_key_a.id, content_key_b.id);
    assert_eq!(content_key_a.content_key, content_key_b.content_key);

    Ok(())
}

#[tokio::test]
async fn test_content_key_create_and_read() -> Result<(), DbErr> {
    let conn = setup_database().await?;

    let content_key_raw: Vec<u8> = vec![
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];

    let content_key_active_model = contentkey::ActiveModel {
        id: NotSet,
        content_key: Set(content_key_raw.clone()),
        created_at: Set(Utc::now()),
    };

    let content_key_model = content_key_active_model.insert(&conn).await?;
    println!("Inserted: content_id={:?}", content_key_model);

    let content_id = contentkey::Entity::find()
        .filter(contentkey::Column::ContentKey.eq(content_key_raw.clone()))
        .one(&conn)
        .await?
        .unwrap();
    assert_eq!(content_id.content_key, content_key_raw);
    Ok(())
}

#[tokio::test]
async fn test_audit_create_and_read() -> Result<(), DbErr> {
    let conn = setup_database().await?;
    // setup the content_key
    let content_key_raw: String =
        String::from("not-a-real-content-key-but-lets-make-sure-its-more-than-32-chars");

    let content_key = contentkey::ActiveModel {
        id: NotSet,
        content_key: Set(content_key_raw.clone().as_bytes().to_vec()),
        created_at: Set(Utc::now()),
    };

    let content_key_active_model = content_key.insert(&conn).await?;
    println!("Inserted: content_key={:?}", content_key_active_model);

    let content_key_model = contentkey::Entity::find()
        .filter(contentkey::Column::ContentKey.eq(content_key_raw.clone().as_bytes().to_vec()))
        .one(&conn)
        .await?
        .unwrap();

    assert_eq!(content_key_model.content_key, content_key_raw.as_bytes().to_vec());

    // setup the content_audit
    let content_audit_active_model = contentaudit::ActiveModel {
        id: NotSet,
        content_key: Set(content_key_model.id),
        created_at: Set(Utc::now()),
        result: Set(contentaudit::AuditResult::Success),
    };

    let content_audit_model = content_audit_active_model.insert(&conn).await?;
    println!("Inserted: content_audit={:?}", content_audit_model);

    let fetched_content_audit_model = contentaudit::Entity::find_by_id(content_audit_model.id)
        .one(&conn)
        .await?
        .unwrap();

    assert_eq!(fetched_content_audit_model.content_key, content_key_model.id);
    assert_eq!(fetched_content_audit_model.result, contentaudit::AuditResult::Success);

    Ok(())
}
