define USAGE
Usage:
	make build run-node
	... in another window: make populate-keys
	open https://polkadot.js.org/apps/#/accounts
	goto > Settings > Developer, add contents of pallets/aggregator/types.json
endef
export USAGE

# keys pinched from https://docs.substrate.io/tutorials/v3/private-network/#add-keys-to-keystore
define KEYSTORE_POPULATE_PAYLOAD
{
	"jsonrpc": "2.0",
	"method": "author_insertKey",
	"params": ["aggr","clip organ olive upper oak void inject side suit toilet stick narrow","0x9effc1668ca381c242885516ec9fa2b19c67b6684c02a8a3237b6862e5c8cd7e"],
	"id": 1
}
endef
export KEYSTORE_POPULATE_PAYLOAD

export RUST_BACKTRACE=1

all: build test clippy

clean:
	cargo clean

clean-aggregator:
	cargo clean -p node-template

build:
	cargo build

build-aggregator: clean-aggregator build

test:
	cargo test

clippy:
	@echo ...should run: cargo clippy
	@echo but due to bug in Rust 1.58.1, only running clippy on the pallets
	pushd pallets/aggregator && cargo clippy ; popd

run-node: build
	target/debug/node-template --dev --tmp

run:
	cargo run -- --dev --tmp

usage:
	@echo "$$USAGE"

populate-keys:
	curl --location --request POST 'http://localhost:9933' \
		--header 'Content-Type: application/json' \
		--data-raw "$$KEYSTORE_POPULATE_PAYLOAD"

benchmark:
	@echo FIXME: failed to default to --wasm-execution compiled, using instead --wasm-execution interpreted-i-know-what-i-do...
	cargo run --manifest-path node/Cargo.toml --features runtime-benchmarks -- benchmark --extrinsic '*' --pallet pallet_aggregator --wasm-execution interpreted-i-know-what-i-do --output ./pallets/aggregator/src/weights.rs  --template=frame-weight-template.hbs

.PHONY: all
