all: $(foreach example,.using_macro .hand_written,$(patsubst %,run.%.$(example),debug release))

release.FLAGS := --release
hand_written.FLAGS := --no-default-features

profile.FLAGS = $($(word 2,$(subst ., ,$@)).FLAGS)
example.FLAGS = $($(word 3,$(subst ., ,$@)).FLAGS)

run.%:
	@cd native-module && cargo build $(FLAGS) $(profile.FLAGS) $(example.FLAGS)
	@cd module-loader && cargo run $(FLAGS) $(profile.FLAGS) $(example.FLAGS)
