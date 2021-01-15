all: run.debug run.release

debug.FLAGS :=
release.FLAGS := --release

run.%:
	@cd native-module && cargo build $(FLAGS) $($*.FLAGS)
	@cd module-loader && cargo run $(FLAGS) $($*.FLAGS)
