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
- "`machine-b`"(with display)
    - Browser listening to `glados-web`

All commands are issued on `machine-a` unless otherwise stated.

Important notes:
- If `glados-monitor`, `glados-audit` and `glados-web` are in `sqlite::memory:`-mode they won't be able to share a database.
- The database used is ephemeral (for testing) and will be removed if the machine reboots.

Start Ethereum execution client (not covered here).

Make an empty database file for glados.
```command
$ touch /tmp/glados-ephemeral.sqlite
```
Start trin (requires https://github.com/ethereum/trin/pull/510), see repo for latest docs.
```command
~/trin$ RUST_LOG=debug cargo run -p trin -- \
    --no-stun \
    --web3-transport ipc \
    --trusted-provider local \
    --local-provider-url 127.0.0.1:8545 \
    --discovery-port 9009 \
    --bootnodes default
```
Start `glados-monitor` (requires https://github.com/pipermerriam/glados/pull/31)
```command
~/glados$ RUST_LOG=glados_monitor=debug cargo run -p glados-monitor -- \
    --database-url sqlite:////tmp/glados-ephemeral.sqlite \
    follow-head \
    --provider-url http://127.0.0.1:8545
```
Start `glados-audit`
```command
~/glados$ RUST_LOG=debug cargo run -p glados-audit -- \
    --ipc-path /tmp/trin-jsonrpc.ipc \
    --database-url sqlite:////tmp/glados-ephemeral.sqlite
```
Start `glados-web`
```command
~/glados$ RUST_LOG=debug cargo run -p glados-web -- \
    --database-url sqlite:////tmp/glados-ephemeral.sqlite
```

On `machine-b`, listen for `glados-web`
```command
$ ssh -N -L 3001:127.0.0.1:3001 <user>@<host>
```
On `machine-b`, navigate to http://127.0.0.1:3001 to view glados audit.