# web3-this-then-that

Subscribe to a Web3 RPC and do something when an event is seen

For now, this is hard coded for LlamaNodes Payment Factory deposit events,
but it could be useful generally.

## Running

```bash
export W3TTT_PROXY_URLS=eth.llamarpc.com,polygon.llamarpc.com
cargo run
```

## Development

The README.md is kept up-to-date with `cargo readme > README.md`

## Questions (and hopefully Answers)

- should this care about confirmation depth?
- should this care about orphaned transactions?

## Todo

- [x] proper logging
- [ ] petgraph for tracking forks?
- [x] retry rather than exit
- [ ] handle orphaned transactions