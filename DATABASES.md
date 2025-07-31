## Databases

Descriptions of the databases used in glados

## Overview

- Tables are defined in migration files (`./migration/src/m*.rs`). Tables are Enums a Migration::up() uses to manage the tables.
- sea_orm uses to migration files to generate `entities` that can be found in `./entities/src/*.rs`, one file per table.
- Entities exist as `Model` structs, whose members are columns in the table.

## Terminology

- Content key
    - A way to refer to a specific content.
    - Based on an existing unique identifier, such as a block number.
    - Is combined with a selector, allowing two elements from the same block to have different
    content keys. (e.g., body and receipts, which otherwise have the same block number).
    - See [spec](https://github.com/ethereum/portal-network-specs/blob/master/history/history-network.md#data-types).
- Content id
    - An identifier that is useful in coordinating content in the network.
    - Derived from the content key.
    - See [spec](https://github.com/ethereum/portal-network-specs/blob/master/history/history-network.md#content-id-derivation-function).
- History data.
    - Each of these is referred to using a content key, and coordinated via content ids.
    - Glados doesn't store everything, only the content keys, content ids and a
    blocknumber/blockhash to make content keys understable.
    - Components
        - Body
            - Transactions, uncles, and withdrawals
        - Receipts
            - Transaction receipts (different from transactions)
- Audit
    - The main function of glados.
    - Steps:
        1. Pick something (a content key), and check the portal network for it (`glados-audit`)
        2. Record a pass/fail and timestamp for that thing.
        3. Display that (`glados-web`), e.g., the content key "`x`" (which may represent "block `y` receipts") was not present in the portal network at timestamp `z`.
- Census
    - Traversing and checking availability of every node on the network
    - Runs periodically

## Definitions

### Tables

- `client`
    - Information about Portal Client that is used for audit
- `node`
    - Unique identifier (`NodeId`) for difference nodes on the network
- `node_enr`
    - Stores ENRs ([Ethereum Node Record](https://github.com/ethereum/devp2p/blob/master/enr.md)) - Information about node's endpoint (public key, IP address, ports, purpose, etc.)
    - Foreign keys:
        - `node_enr.node_id` -> `node.id`
- `census`
    - The information about each individual census of the network
- `census_node`
    - Information about nodes discovered during census
    - Foreign keys:
        - `census_node.census_id` -> `census.id`
        - `census_node.node_enr_id` -> `node_enr.id`
            - The ENR of the node during census
- `content`
    - Information about content (content key, content id, block number, ...)
- `audit`
    - Details about performed audits (timestamp, result, trace, ...)
    - Trace is stored if any failure is detected
    - Foreign keys:
        - `audit.content_id` -> `content.id`
        - `audit.client_id` -> `client.id`
            - Portal client used for audit
        - `audit.node_id` -> `node.id`
            - `NodeId` of the portal client used for audit
- `audit_transfer_failure`
    - Information about content transfer failure that happened during audit
    - The audit itself could have been successful if contetn was received from some other node
    - Foreign keys:
        - `audit_transfer_failure.audit_id` -> `audit.id`
        - `audit_transfer_failure.sender_node_enr_id` -> `node_enr.id`
- `audit_latest`
    - Status of the last audit per content
    - Updated automatically after each audit
    - Foreign keys:
        - `audit_latest.content_id` -> `content.id`
        - `audit_latest.audit.id` -> `audit.id`
- `audit_stats`
    - The audit stats over 1h period
    - Calculated by the background task periodically (15 min by default)
