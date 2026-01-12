check:
	cargo clippy --all-targets --features full -- -D warnings

clean:
	rm -rf ./target

fix:
	cargo fix --allow-dirty
	cargo clippy --fix --allow-dirty
	cargo fmt

test:
	cargo test

.PHONY: check clean fix test
