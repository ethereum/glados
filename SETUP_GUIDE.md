## Setup guide

For specific configurations.
### Local testing environment

This example uses the following:
- "`machine-a`" (headless Ubuntu)
    - Locally running Ethereum execution client
    - `trin`
    - `glados-audit`
    - `glados-web`
- "`machine-b`" (with display)
    - Browser listening to `glados-web`

All commands are issued on `machine-a` unless otherwise stated.

Start trin (see [docs](https://ethereum.github.io/trin/developers/quick_setup.html))
```command
~/trin$ RUST_LOG=debug cargo run -p trin -- --web3-transport http
```

Start `glados-audit`, which takes monitoring data from the glados database,
checks if `trin` has it, then records the outcome in the glados database.
```command
~/glados$ RUST_LOG=debug cargo run -p glados-audit -- \
    --portal-client http://127.0.0.1:8545 \
    --database-url postgres://<user>:<password>@localhost:5432/<database> \
    --strategy random
```

Start `glados-web`, which takes audit data from the glados database and serves
that for viewing.
```command
~/glados$ RUST_LOG=debug cargo run -p glados-web -- \
    --database-url postgres://<user>:<password>@localhost:5432/<database>
```

Start `glados-cartographer`, which takes census of all the nodes on the network
```command
~/glados$ RUST_LOG=debug cargo run -p glados-cartographer -- \
  --transport http  --http-url http://127.0.0.1:8545  \
  --database-url postgres://<user>:<password>@localhost:5432/<database>
  
```

On `machine-b`, listen for `glados-web`
```command
$ ssh -N -L 3001:127.0.0.1:3001 <user>@<host>
```
On `machine-b`, navigate to http://127.0.0.1:3001 to view glados audit.
