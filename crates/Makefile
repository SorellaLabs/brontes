.PHONY: mainnet arbitrum mainnet-release arbitrum-release

mainnet:
	cargo build

arbitrum:
	cargo build --features arbitrum

mainnet-release:
	RUSTFLAGS="-C target-cpu=native" cargo build --profile maxperf

arbitrum-release:
	RUSTFLAGS="-C target-cpu=native" cargo build --features arbitrum --profile maxperf