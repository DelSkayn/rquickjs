patch = all #hotfix msvc exports

$(info patch = $(patch))

ifneq ($(filter hotfix all,$(patch)),)
PATCHES += get_function_proto
PATCHES += check_stack_overflow
endif

# apply patches
apply:
	@cd quickjs $(foreach PATCH,$(PATCHES), && echo '*** $(PATCH) ***' && patch -p1 < ../patches/$(PATCH).patch)

# stage changes
stage:
	@cd quickjs && git add *

# show unstaged diff
diff:
	@cd quickjs && git diff -w

# update patch from unstaged changes
update.%:
	@cd quickjs && git diff -w > ../patches/$*.patch

# reset patches
reset:
	@cd quickjs && git reset --hard HEAD
