## Databases
Descriptions of the databases used in glados

## Overview

- Tables are defined in migration files (`./migration/src/m*.rs`). Tables are Enums a Migration::up() uses to manage the tables.
- sea_orm uses to migration files to generate `entities` that can be found in `./entities/src/*.rs`, one file per table.
- Entities exist as `Model` structs, whose members are columns in the table.

## Terminology

- Content key
    - A way to refer to a specific element.
    - Based on an existing unique identifier, such as a block hash or transaction hash.
    - Is combined with a selector, allowing two elements from the same block to have different
    content keys. (e.g., body and receipts, which otherwise have the same block hash).
    - See [spec](https://github.com/ethereum/portal-network-specs/blob/master/history-network.md#data-types).
- Content id
    - An identifier that is useful in coordinating content in the network.
    - Derived by hashing the content key.
    - See [spec](https://github.com/ethereum/portal-network-specs/blob/master/portal-wire-protocol.md#content-keys-and-content-ids).
- History data.
    - Each of these is referred to using a content key, and coordinated via content ids.
    - Glados doesn't store everything, only the content keys, content ids and a
    blocknumber/blockhash to make content keys understable.
    - Components
        - Header
            - Everything in a block except the body and receipts.
        - Body
            - Transactions and uncles
        - Receipts
            - Transaction receipts (different from transactions)
- Audit
    - The main function of glados.
    - Consists of a content key, a pass/fail and a timestamp.
    - Steps:
        1. Record what the portal network should have (`glados-monitor`)
        2. Pick something (a content key), and check the portal network for it (`glados-audit`)
        3. Record a pass/fail and timestamp for that thing.
        4. Display that (`glados-web`), e.g., the content key "`x`" (which may represent "block `y`
        receipts") was not present in the portal network at timestamp `z`.

## Definitions

Defined in `/src/migration/`:
- `m20220101_000001_create_table.rs`
    - Node
    - Record
    - KeyValue
- `m20221114_143914_create_content_id_key_and_audit.rs`
    - ContentKey
    - ContentAudit
- `m20230125_205211_create_execution_header_table.rs`
    - ExecutionHeader
- `m20230127_162559_create_execution_body_table.rs`
    - ExecutionBody
- `m20230127_162626_create_execution_receipts_table.rs`
    - ExecutionReceipts

## Relationships

Tables have the following relationships:
- Node
    - Record
        - KeyValue
- ContentKey
        - ContentAudit (glados audit results)
    - ExecutionHeader (data for context)
    - ExecutionBody (data for context)
    - ExecutionReceipts (data for context)

## Contents

Tables have the following columns:

- Node
    - NodeId
- Record
    - NodeId
    - SequenceNumber
    - Raw
    - CreatedAt
- KeyValue
    - RecordId
    - Key
    - Value
- ContentKey
    - ContentKey
    - CreatedAt
- ContentAudit
    - ContentKey
    - CreatedAt
    - Result
- ExecutionHeader
    - ContentKey
    - BlockNumber
    - BlockHash
- ExecutionBody
    - ContentKey
    - BlockNumber
    - BlockHash
- ExecutionReceipts
    - ContentKey
    - BlockNumber
    - BlockHash
