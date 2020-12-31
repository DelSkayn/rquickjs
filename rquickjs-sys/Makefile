patch = all #hotfix msvc exports parallel

$(info patch = $(patch))

ifneq ($(filter hotfix all,$(patch)),)
PATCHES += check_stack_overflow
PATCHES += infinity_handling
PATCHES += atomic_new_class_id
endif

ifneq ($(filter msvc all,$(patch)),)
PATCHES += basic_msvc_compat
endif

ifneq ($(filter exports all,$(patch)),)
PATCHES += read_module_exports
endif

ifneq ($(filter parallel all,$(patch)),)
PATCHES += reset_stack_pointer
endif

apply:
	@cd quickjs $(foreach PATCH,$(PATCHES), && echo '*** $(PATCH) ***' && patch -p1 < ../patches/$(PATCH).patch)

stage:
	@cd quickjs && git add *

diff:
	@cd quickjs && git diff -w

reset:
	@cd quickjs && git reset --hard HEAD
