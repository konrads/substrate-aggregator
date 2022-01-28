# DeFI Aggregator pallet

This is an implementation of a DeFI aggregator, comprising:
- parametrizable longest path algorithm
- OCW worker fetching latest price data, run the longest path algorithm, record results via extrinsics
- extrinsic interface for:
  - internal usage, eg. recording of latest price data
  - admin/root, eg. addition/deletion of price pairs (by provider)
  - trading API utilizing the latest price data

## Details
This pallet comprises following structure:
```
  *
  |
  +---- lib.rs
  |
  +---- types.rs
  |
  +---- utils.rs
  |
  +---- heap.rs
  |
  +----+ best_path_calculator
  |    |
  |    +----- noop
  |    |
  |    +----- floyd_warshall
  |
  +----+ trade_provider
       |
       +----- crypto_compare
```
- [lib.rs](src/lib.rs) - OCW mechanisms and extrinsic APIs
- [types.rs](src/types.rs) - types utilized throughout
- [utils.rs](src/utils.rs) - common utils
- [heap.rs](src/heap.rs) - heap implementation
- [best_path_calculator/floyd_warshall](best_path_calculator/floyd_warshall) - implementation of a shortest/longest path algorithm, comprising:
  - Floyd-Warshall implementation as per https://www.youtube.com/watch?v=oNI0rf2P9gE&ab_channel=AbdulBari
  - longest path implementation with weight multiplications, as per https://www.coursera.org/lecture/algorithms-on-graphs/currency-exchange-reduction-to-shortest-paths-cw8Tm
- [trade_provider/crypto_compare] - price data fetcher, as lifted from OCW example

simple example, assumes only provided pairs are reachable

### Longest path algorithm
For longest path calculations, Floyd-Warshall algorithm was chosen for its ability to calculate shortest/longest paths across all vertices.

For multiplication based weights, it's been noticed that product maximisation is equivalent to maximisation of log of weights, as per: `x*y = 2^(log2(x) + log2(y))`.

For longest paths, weights have been multiplied by `-1` and hence reused in shortest path algorithm.

*NOTE:* Floyd-Warshall can detect negative path cycles (ie. infinite arbitrage opportunities), which cause the latest price update to be ignored. Potential FIXME - remove offending edge to remove negative cycles...

### OCW
OCW triggers price fetching, best path calculation, compares with currently stored best path and issues updates via unsigned root origin extrinsic.

OCW trigger is guarded by an `OffchainTriggerFreq` constant ensuring price fetching doesn't happen too frequently, as well as `AcceptNextOcwTxAt` storage / `UnsignedTxAcceptFreq` constant ensuring unsigned transactions are received too frequently.

### API
- admin (root origin)
  - `ocw_submit_best_paths_changes()` - for price change delta submissions from onchain
  - `add_price_pair()`/`delete_price_pair()`/`submit_price_pairs()` - for submission of to-be-monitored price pairs by provider
- public facing
  - `trade()` - to perform trade as per chosen trade path (not yet integrated with a provider)

### Storage
In flux, to-be-described...

### Events
In flux, to-be-described...

### Constants
- `OffchainTriggerFreq` - determines OCW trigger frequency
- `UnsignedTxAcceptFreq` - determines unsigned transaction receipt frequency
- `UnsignedPriority` - sets unsigned transaction priority (to play nicely with other pallets)
- `PriceChangeTolerance` - sets acceptable price change tolerance, if not breached, on-chain prices aren't updated

## Usage
```bash
make build          # build pallet/runtime
make test           # verify build
make clippy         # ensure code quality

make run            # start the project
make run-node       # start the project, from pre-compiled node
make populate-keys  # upload keys, once node stabilizes

```

## Snags/TODOs
| Stage | Description                                                                                                                                                | Status |
| ------| ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
|   1   | Implement weights, including API allowing for submission of multiple price data                                                                            |   𐄂    |
|   1   | Bootstrap storage to allow for configuration for price pairs per provider (currently needs root origin extrinsic invocations)                              |   𐄂    |
|   1   | Investigate keys bootstrap (currently done with curl, see above)                                                                                           |   𐄂    |
|   1   | Consider abstracting Cost (aka Amount) from Balance to allow for more elaborate cost calculations, including transaction fees, slippage, etc               |   𐄂    |
|   2   | Split repository into a) pallete branch b) runtime branch                                                                                                  |   𐄂    |
|   2   | Deploy on testnet                                                                                                                                          |   𐄂    |
|   2   | Construct Angular UI (to reside on separate branch)                                                                                                        |   𐄂    |
|   3   | Revise mechanisms for submission of internal price data, ie. with what origin, signed/unsigned transaction, signed/unsigned payload, signed with a refund? |   𐄂    |
|   3   | Utilize XCM to plug into a real price/trade provider                                                                                                       |   𐄂    |
