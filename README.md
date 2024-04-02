# Glados

Network health monitoring tool for the Portal Network


## Project Overview

The project is split up into a few different crates.

- `glados-core`: Contains shared code that is shared by the other crates.
- `glados-web`: The web application that serves the HTML dashboard
- `glados-monitor`: The long running system processes that pull in chain data and audit the portal network.

### Technology Choices

- [`sea-orm`](https://docs.rs/sea-orm/latest/sea_orm/) - ORM and database migrations.  The `entity` and `migration` crates are sea-orm conventions.
- [`axum`](https://docs.rs/axum/latest/axum/) - Web framework for serving HTML.
- [`askama`](https://djc.github.io/askama/) - Templating for HTML pages.
- [`web3`](https://docs.rs/web3/latest/web3/) - For querying an Ethereum provider for chain data
- [`tokio`](https://tokio.rs/) - Async runtime.
- [`tracing`](https://docs.rs/tracing/latest/tracing/) - Structured logging

For our database, we use Postgres in both development and production.

### Architecture

The rough shape of Glados is as follows:

The `glados-monitor` crate implements a long running process which continually follows the tip of the chain, and computes the ContentID/ContentKey values for new content as new blocks are added to the canonical chain.  These values are inserted into a relational database.

The `glados-audit` process then queries the database for content that it will then "audit" to determine whether the content can be successfully retrieved from the network.  The audit process will use the Portal Network JSON-RPC api to query the portal network for the given content and then record in the database whether the content could be successfully retrieved.  The database is structured such that a piece of content can be audited many times, giving a historical view over the lifetime of the content showing times when it was or was not available.

The `glados-web` crate implements a web application to display information from the database about the audits.  The goal is to have a dashboard that provides a single high level overview of the network health, as well as the ability to drill down into specific pieces of content to see the individual audit history.


## Running Things

For specific examples, see the [SETUP_GUIDE.md](/SETUP_GUIDE.md).

### Quick Deploy via Docker:

See the [DOCKER_GUIDE.md](/DOCKER_GUIDE.md)

### Basics

Glados needs a postgres database to use. To run a postgres instance locally using docker:

`docker run --name postgres -e POSTGRES_DB=glados POSTGRES_PASSWORD=password -d -p 5432:5432 postgres`

This postgres instance can be accessed via `postgres://postgres:password@localhost:5432/glados`. This value will be referred to as the `DATABASE_URL`.

In most cases, you will want to set the environment variable `RUST_LOG` to enable some level of `debug` level logs.  `RUST_LOG=glados_monitor=debug` is a good way to only enable the debug logs for a specific crate/namespace.

### Running `glados-monitor`

The `glados-monitor` crate can be run as follows to populate a local database with content ids.

The CLI needs a DATABASE_URL to know what relational database to connect to, as well as an HTTP_PROVIDER_URI to connect to an Ethereum JSON-RPC provider (not a portal node).

```
$ cargo run -p glados-monitor -- --database-url <DATABASE_URL> follow-head --provider-url <HTTP_PROVIDER_URI>
```
For example, if an Ethereum execution client is running on localhost port 8545:
```
$ cargo run -p glados-monitor -- --database-url  follow-head --provider-url http://127.0.0.1:8545
```
#### Importing the pre-merge accumulators

The pre-merge epoch accumulators can be found here: https://github.com/njgheorghita/portal-accumulators

They can be imported with this command

```
$ cargo run -p glados-monitor -- --database-url <DATABASE_URL> import-pre-merge-accumulators --path /path/to/portal-accumulators/bridge_content
```

### Running `glados-web`


The CLI needs a DATABASE_URL to know what relational database to connect to.

> This has only been tested using the `trin` portal network client.

```
$ cargo run -p glados-web -- --database-url DATABASE_URL
```

You should then be able to view the web application at `http://127.0.0.1:3001/` in your browser.
