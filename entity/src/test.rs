#![allow(unused_imports)]
use std::str::FromStr;

use alloy_primitives::{B256, U256};
#[cfg(test)]
use chrono::prelude::*;
use enr::NodeId;
#[cfg(test)]
use ethportal_api::types::node_id::generate_random_node_id;
use ethportal_api::{BlockHeaderKey, HistoryContentKey, OverlayContentKey};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Database, DbConn, DbErr, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, Set,
};

use ethportal_api::utils::bytes::hex_encode;
use migration::{Migrator, MigratorTrait};

use crate::content::SubProtocol;
use crate::content_audit::{HistorySelectionStrategy, SelectionStrategy};
use crate::{client_info, content, content_audit, node, record};
use pgtemp::PgTempDB;

#[allow(dead_code)]
// Temporary Postgres db will be deleted once PgTempDB goes out of scope, so keep it in scope.
pub async fn setup_database() -> Result<(DbConn, PgTempDB), DbErr> {
    let db: PgTempDB = PgTempDB::async_new().await;
    let conn: DbConn = Database::connect(db.connection_uri()).await?;
    Migrator::up(&conn, None).await.unwrap();

    Ok((conn, db))
}

#[tokio::test]
async fn test_node_crud() -> Result<(), DbErr> {
    let (conn, _db) = setup_database().await?;

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
        node_id_high: Set(0),
    };
    let node_b = node::ActiveModel {
        id: NotSet,
        node_id: Set(node_id_b.clone()),
        node_id_high: Set(0),
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
    use enr::{k256, Enr};
    use rand::thread_rng;
    use std::net::Ipv4Addr;

    // generate a random secp256k1 key
    let mut rng = thread_rng();
    let key = k256::ecdsa::SigningKey::random(&mut rng);

    let ip = Ipv4Addr::new(192, 168, 0, 1);
    let enr = {
        let mut builder = Enr::builder();
        builder.ip4(ip).tcp4(8000).build(&key).unwrap()
    };

    assert_eq!(enr.ip4(), Some("192.168.0.1".parse().unwrap()));
    assert_eq!(enr.id(), Some("v4".into()));

    Ok(())
}

#[allow(dead_code)]
/// Returns a history content key representing the header with proof
/// for block hash `0x0001...1e1f`
fn sample_history_key() -> HistoryContentKey {
    let block_hash: [u8; 32] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ];
    HistoryContentKey::BlockHeaderWithProof(BlockHeaderKey { block_hash })
}

/// Tests that the database helper method id_as_hash() works.
#[tokio::test]
async fn test_content_id_as_hash() -> Result<(), DbErr> {
    let (conn, _db) = setup_database().await?;
    let key = sample_history_key();
    let content_id_hash = B256::from_slice(&key.content_id());
    let content_model = content::get_or_create(SubProtocol::History, &key, Utc::now(), &conn)
        .await
        .unwrap();
    assert_eq!(content_model.id_as_hash(), content_id_hash);
    Ok(())
}

/// Tests that the database helper method id_as_hex() works.
#[tokio::test]
async fn test_content_id_as_hex() -> Result<(), DbErr> {
    let (conn, _db) = setup_database().await?;
    let key = sample_history_key();
    let content_id_hash = B256::from_slice(&key.content_id());
    let content_id_hex = hex_encode(content_id_hash);
    let content_model = content::get_or_create(SubProtocol::History, &key, Utc::now(), &conn)
        .await
        .unwrap();
    assert_eq!(content_model.id_as_hex(), content_id_hex);
    Ok(())
}

/// Tests that the database helper method key_as_hex() works.
#[tokio::test]
async fn test_content_key_as_hex() -> Result<(), DbErr> {
    let (conn, _db) = setup_database().await?;
    let key = sample_history_key();
    let content_model = content::get_or_create(SubProtocol::History, &key, Utc::now(), &conn)
        .await
        .unwrap();
    assert_eq!(
        content_model.key_as_hex(),
        "0x00000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
    );
    Ok(())
}

/// Tests that the get_or_create() function correctly handles the
/// presence or absence of a key in the database.
#[tokio::test]
async fn test_content_get_or_create() -> Result<(), DbErr> {
    let (conn, _db) = setup_database().await?;
    let key = sample_history_key();

    // Ensure our database is empty
    assert_eq!(content::Entity::find().count(&conn).await?, 0);

    let content_id_a = content::get_or_create(SubProtocol::History, &key, Utc::now(), &conn)
        .await
        .unwrap();

    // Ensure we added a new record to the database.
    assert_eq!(content::Entity::find().count(&conn).await?, 1);

    // Retrieve the key
    let content_id_b = content::get_or_create(SubProtocol::History, &key, Utc::now(), &conn)
        .await
        .unwrap();

    // Key was not saved twice.
    assert_eq!(content::Entity::find().count(&conn).await?, 1);

    // Ensure that get_or_create found the existing entry.
    assert_eq!(content_id_a.id, content_id_b.id);
    assert_eq!(content_id_a.content_key, content_id_b.content_key);
    Ok(())
}

#[tokio::test]
async fn test_audit_crud() -> Result<(), DbErr> {
    let (conn, _db) = setup_database().await?;
    let key = sample_history_key();

    let content_key_active_model = content::ActiveModel {
        id: NotSet,
        content_id: Set(key.content_id().to_vec()),
        content_key: Set(key.to_bytes()),
        protocol_id: Set(SubProtocol::History),
        first_available_at: Set(Utc::now()),
    };

    let content_model = content_key_active_model.insert(&conn).await?;

    let searched_content_model = content::Entity::find()
        .filter(content::Column::ContentId.eq(key.content_id().to_vec()))
        .one(&conn)
        .await?
        .unwrap();

    assert_eq!(searched_content_model.content_id, key.content_id());
    assert_eq!(searched_content_model.content_key, key.to_bytes());

    let client_info_active_model = client_info::ActiveModel {
        id: NotSet,
        version_info: Set("trin v0.1.0".to_owned()),
    };
    let client_info_model = client_info_active_model.insert(&conn).await?;

    let node_id = NodeId::random();

    let node = node::get_or_create(node_id, &conn).await.unwrap();

    // setup the content_audit
    let content_audit_active_model = content_audit::ActiveModel {
        id: NotSet,
        content_key: Set(searched_content_model.id),
        created_at: Set(Utc::now()),
        strategy_used: Set(Some(SelectionStrategy::History(
            HistorySelectionStrategy::Random,
        ))),
        result: Set(content_audit::AuditResult::Success),
        trace: Set("".to_owned()),
        client_info: Set(Some(client_info_model.id)),
        node: Set(Some(node.id)),
    };

    let content_audit_model = content_audit_active_model.insert(&conn).await?;

    let searched_content_audit_model = content_audit::Entity::find_by_id(content_audit_model.id)
        .one(&conn)
        .await?
        .unwrap();

    assert_eq!(searched_content_audit_model.content_key, content_model.id);
    assert_eq!(
        searched_content_audit_model.result,
        content_audit::AuditResult::Success
    );
    assert_eq!(
        searched_content_audit_model.strategy_used,
        Some(content_audit::SelectionStrategy::History(
            HistorySelectionStrategy::Random
        ))
    );

    Ok(())
}

/// Tests that the content table unique constraints prevent duplicate entries.
/// No two keys should have the same protocol_id, content_key and content_id combination.
#[tokio::test]
async fn test_content_table_unique_constraints() {
    let (conn, _db) = setup_database().await.unwrap();
    let id_a = vec![1; 32];
    let id_b = vec![2; 32];
    let key_a = vec![3; 32];
    let key_b = vec![4; 32];
    let protocol_a = SubProtocol::History;
    let protocol_b = SubProtocol::State;
    // DB=0. Add one key (accepts). DB==1.
    let action_a = content::ActiveModel {
        id: NotSet,
        content_id: Set(id_a.clone()),
        content_key: Set(key_a.clone()),
        protocol_id: Set(protocol_a.clone()),
        first_available_at: Set(Utc::now()),
    };
    action_a.clone().insert(&conn).await.unwrap();
    assert_eq!(content::Entity::find().count(&conn).await.unwrap(), 1);

    // DB=1. Repeat addition (rejects). DB=1.
    assert!(action_a
        .insert(&conn)
        .await
        .unwrap_err()
        .to_string()
        .contains("violates unique constraint"));

    assert_eq!(content::Entity::find().count(&conn).await.unwrap(), 1);

    // DB=1. Add same content_key, same content_id, different protocol (accepts). DB=2.
    let action_b = content::ActiveModel {
        id: NotSet,
        content_id: Set(id_a.clone()),
        content_key: Set(key_a.clone()),
        protocol_id: Set(protocol_b),
        first_available_at: Set(Utc::now()),
    };
    action_b.clone().insert(&conn).await.unwrap();
    assert_eq!(content::Entity::find().count(&conn).await.unwrap(), 2);

    // DB=2. Repeat addition (rejects). DB=2.
    assert!(action_b
        .insert(&conn)
        .await
        .unwrap_err()
        .to_string()
        .contains("violates unique constraint"));
    assert_eq!(content::Entity::find().count(&conn).await.unwrap(), 2);

    // DB=2. Add same content_key, different content_id, same protocol (rejects). DB=2.
    let action_c = content::ActiveModel {
        id: NotSet,
        content_id: Set(id_b),
        content_key: Set(key_a),
        protocol_id: Set(protocol_a.clone()),
        first_available_at: Set(Utc::now()),
    };
    assert!(action_c
        .clone()
        .insert(&conn)
        .await
        .unwrap_err()
        .to_string()
        .contains("violates unique constraint"));
    assert_eq!(content::Entity::find().count(&conn).await.unwrap(), 2);

    // DB=2. Repeat addition (rejects). DB=2.
    assert!(action_c
        .insert(&conn)
        .await
        .unwrap_err()
        .to_string()
        .contains("violates unique constraint"));
    assert_eq!(content::Entity::find().count(&conn).await.unwrap(), 2);

    // DB=2. Add different content_key, same content_id, same protocol (rejects). DB=2.
    let action_d = content::ActiveModel {
        id: NotSet,
        content_id: Set(id_a),
        content_key: Set(key_b),
        protocol_id: Set(protocol_a),
        first_available_at: Set(Utc::now()),
    };
    assert!(action_d
        .insert(&conn)
        .await
        .unwrap_err()
        .to_string()
        .contains("violates unique constraint"));
    assert_eq!(content::Entity::find().count(&conn).await.unwrap(), 2);
}

#[tokio::test]
async fn test_query_closest() {
    use env_logger;
    env_logger::init();
    let (conn, _db) = setup_database().await.unwrap();

    let node_id_a = NodeId::random();
    let node_id_b = NodeId::random();
    let node_id_c = NodeId::random();

    let node_a = node::get_or_create(node_id_a, &conn).await.unwrap();
    let node_b = node::get_or_create(node_id_b, &conn).await.unwrap();
    let node_c = node::get_or_create(node_id_c, &conn).await.unwrap();

    let distance_a_b = node_a.node_id_high ^ node_b.node_id_high;
    let distance_a_c = node_a.node_id_high ^ node_c.node_id_high;
    let distance_b_c = node_b.node_id_high ^ node_c.node_id_high;

    let node_id_a_full = U256::from_be_slice(&node_id_a.raw());
    let node_id_b_full = U256::from_be_slice(&node_id_b.raw());
    let node_id_c_full = U256::from_be_slice(&node_id_c.raw());

    assert_eq!(
        node_a.node_id_high as u64,
        node_id_a_full.wrapping_shr(193).to::<u64>()
    );
    assert_eq!(
        node_b.node_id_high as u64,
        node_id_b_full.wrapping_shr(193).to::<u64>()
    );
    assert_eq!(
        node_c.node_id_high as u64,
        node_id_c_full.wrapping_shr(193).to::<u64>()
    );

    let distance_a_b_full = node_id_a_full ^ node_id_b_full;
    let distance_a_c_full = node_id_a_full ^ node_id_c_full;
    let distance_b_c_full = node_id_b_full ^ node_id_c_full;

    //let distance_a_b_alt = (node_id_a_full | node_id_b_full) - (node_id_a_full & node_id_b_full);

    assert_eq!(
        distance_a_b_full.wrapping_shr(193).to::<u64>(),
        distance_a_b as u64
    );
    assert_eq!(
        distance_a_c_full.wrapping_shr(193).to::<u64>(),
        distance_a_c as u64
    );
    assert_eq!(
        distance_b_c_full.wrapping_shr(193).to::<u64>(),
        distance_b_c as u64
    );

    assert_eq!(
        distance_a_b_full > distance_a_c_full,
        distance_a_b > distance_a_c
    );
    assert_eq!(
        distance_a_c_full > distance_b_c_full,
        distance_a_c > distance_b_c
    );
    assert_eq!(
        distance_a_b_full > distance_b_c_full,
        distance_a_b > distance_b_c
    );

    let nodes_near_a = node::closest_xor(node_id_a, &conn).await.unwrap();
    assert_eq!(nodes_near_a.len(), 3);

    let expected_distances_a = match distance_a_b > distance_a_c {
        true => [0, distance_a_c, distance_a_b],
        false => [0, distance_a_b, distance_a_c],
    };
    let actual_distances_a = [
        nodes_near_a[0].distance,
        nodes_near_a[1].distance,
        nodes_near_a[2].distance,
    ];
    assert_eq!(expected_distances_a, actual_distances_a);

    let expected_from_a = match distance_a_b > distance_a_c {
        true => [node_a.id, node_c.id, node_b.id],
        false => [node_a.id, node_b.id, node_c.id],
    };
    let order_from_a = [nodes_near_a[0].id, nodes_near_a[1].id, nodes_near_a[2].id];
    assert_eq!(order_from_a, expected_from_a);

    let nodes_near_b = node::closest_xor(node_id_b, &conn).await.unwrap();
    let expected_from_b = match distance_a_b > distance_b_c {
        true => [node_b.id, node_c.id, node_a.id],
        false => [node_b.id, node_a.id, node_c.id],
    };
    let order_from_b = [nodes_near_b[0].id, nodes_near_b[1].id, nodes_near_b[2].id];
    assert_eq!(order_from_b, expected_from_b);

    let nodes_near_c = node::closest_xor(node_id_c, &conn).await.unwrap();
    let expected_from_c = match distance_a_c > distance_b_c {
        true => [node_c.id, node_b.id, node_a.id],
        false => [node_c.id, node_a.id, node_b.id],
    };
    let order_from_c = [nodes_near_c[0].id, nodes_near_c[1].id, nodes_near_c[2].id];
    assert_eq!(order_from_c, expected_from_c);
}
