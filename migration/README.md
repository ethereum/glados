# Migration commands

## Using `sea-orm-cli`
Install
```command
cargo install sea-orm-cli
```
Create (if none already created) a new blank postgres instance:
`docker run --name postgres -e POSTGRES_DB=glados -e POSTGRES_PASSWORD=password -d -p 5432:5432 postgres`
`export DATABASE_URL=postgres://postgres:password@localhost:5432/glados`
```
Generate all entities. Commands are made from the project root directory.
```command
sea-orm-cli generate entity -o entity/src
```
Generate entity only for `MyNewTable` as defined in a migration (`./migration/src/*.rs`) file:
```command
sea-orm-cli generate entity -o entity/src -t my_new_table
```
Make the tables/changes in the new database:
```command
sea-orm-cli migrate up
```
## Running Migrator CLI

- Generate a new migration file
    ```sh
    cargo run -- generate MIGRATION_NAME
    ```
- Apply all pending migrations
    ```sh
    cargo run
    ```
    ```sh
    cargo run -- up
    ```
- Apply first 10 pending migrations
    ```sh
    cargo run -- up -n 10
    ```
- Rollback last applied migrations
    ```sh
    cargo run -- down
    ```
- Rollback last 10 applied migrations
    ```sh
    cargo run -- down -n 10
    ```
- Drop all tables from the database, then reapply all migrations
    ```sh
    cargo run -- fresh
    ```
- Rollback all applied migrations, then reapply all migrations
    ```sh
    cargo run -- refresh
    ```
- Rollback all applied migrations
    ```sh
    cargo run -- reset
    ```
- Check the status of all migrations
    ```sh
    cargo run -- status
    ```
