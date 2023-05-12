## Setup guide

For specific configurations.
### Local testing environment

This example uses the following:
- "`machine-a`" (headless Ubuntu)
    - Locally running Ethereum execution client
    - `trin`
    - `glados-monitor`
    - `glados-audit`
    - `glados-web`
- "`machine-b`" (with display)
    - Browser listening to `glados-web`

All commands are issued on `machine-a` unless otherwise stated.

Important note:
- If `glados-monitor`, `glados-audit` and `glados-web` are in `sqlite::memory:`-mode they won't be able to share a database. In-memory databases are ephemeral and only persist as long as the process is running.

Start Ethereum execution client (not covered here).

Make an empty database file for glados.
```command
$ touch /path/to/database.sqlite
```
Start trin (see [docs](https://github.com/ethereum/trin/blob/master/docs/ubuntu_guide.md)
```command
~/trin$ RUST_LOG=debug cargo run -p trin -- \
    --no-stun \
    --web3-transport ipc \
    --trusted-provider local \
    --local-provider-url 127.0.0.1:8545 \
    --discovery-port 9009 \
    --bootnodes default
```
Start `glados-monitor`, which uses chain data from the execution node and stores that in the
glados database. For an empty database file, the `--migration` flag triggers
database table creation.
```command
~/glados$ RUST_LOG=glados_monitor=debug cargo run -p glados-monitor -- \
    --migrate \
    --database-url sqlite:////path/to/database.sqlite \
    follow-head \
    --provider-url http://127.0.0.1:8545
```
Start `glados-audit`, which takes monitoring data from the glados database,
checks if `trin` has it, then records the outcome in the glados database.
```command
~/glados$ RUST_LOG=debug cargo run -p glados-audit -- \
    --portal-client /path/to/trin-jsonrpc.ipc \
    --database-url sqlite:////path/to/database.sqlite
```
Start `glados-web`, which takes audit data from the glados database and serves
that for viewing.
```command
~/glados$ RUST_LOG=debug cargo run -p glados-web -- \
    --database-url sqlite:////path/to/database.sqlite
```

On `machine-b`, listen for `glados-web`
```command
$ ssh -N -L 3001:127.0.0.1:3001 <user>@<host>
```
On `machine-b`, navigate to http://127.0.0.1:3001 to view glados audit.
