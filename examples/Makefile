.SUFFIXES:

EXAMPLES := $(shell cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | select(.manifest_path | contains("/examples/")) | .name')
OTHER_EXAMPLES := $(filter-out native-module module-loader rquickjs-cli,$(EXAMPLES))

other: $(addprefix other.,$(OTHER_EXAMPLES))

other.%:
	@echo "Running $*"
	@cd $* && cargo run

cli:
	@echo "Running rquickjs-cli"
	@cd rquickjs-cli && echo "exit()" | cargo run

release.FLAGS := --release
hand_written.FLAGS := --no-default-features

profile.FLAGS = $($(word 2,$(subst ., ,$@)).FLAGS)
variant.FLAGS = $($(word 3,$(subst ., ,$@)).FLAGS)

native: $(foreach variant,.using_macro .hand_written,$(patsubst %,native.%.$(variant),debug release))

native.%:
	@echo "Running native tests with flags: $(FLAGS) $(profile.FLAGS) $(variant.FLAGS)"
	@cd native-module && cargo build $(FLAGS) $(profile.FLAGS) $(variant.FLAGS)
	@cd module-loader && cargo run $(FLAGS) $(profile.FLAGS) $(variant.FLAGS)

run: other native cli
