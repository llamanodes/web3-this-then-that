# web3-this-then-that

Subscribe to a Web3 RPC and do something when an event is seen

Initially, this will be hard coded for web3-proxy deposit transactions,
but it would probably be useful generally.

## Running

```bash
W3TTT_RPC_WS_URL=wss://polygon.llamarpc.com cargo run
```

## Development

The README.md is kept up-to-date with `cargo readme > README.md`

## Questions (and hopefully Answers)

- should this care about confirmation depth?
- should this care about orphaned transactions?

## Todo

- [ ] proper logging
- [ ] moka cache for blocks
- [ ] petgraph for tracking forks?
- [ ] retry rather than exit



