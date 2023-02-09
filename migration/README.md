# Migration commands

## Using `sea-orm-cli`
Install
```command
cargo install sea-orm-cli
```
Create (if none already created) a new blank database file:
```command
touch /path/to/my-database.sqlite
```
Generate all entities. Commands are made from the project root directory.
```command
DATABASE_URL=sqlite:////path/to/my-database.sqlite sea-orm-cli generate entity -o entity/src
```
Generate entity ony for `MyNewTable` as defined in a migration (`./migration/src/*.rs`) file:
```command
DATABASE_URL=sqlite:////path/to/my-database.sqlite sea-orm-cli generate entity -o entity/src -t my_new_table
```
Make the tables/changes in the new database:
```command
DATABASE_URL=sqlite:////path/to/my-database.sqlite sea-orm-cli migrate up
```
## Running Migrator CLI

- Generate a new migration file
    ```sh
    cargo run -- migrate generate MIGRATION_NAME
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
