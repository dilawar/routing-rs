build:
	maturin build

fmt:
	cargo +nightly fmt

clippy lint:
	cargo clippy --no-deps

fix:
	cargo clippy --no-deps --fix --allow-dirty
