patch = all #hotfix msvc exports

$(info patch = $(patch))

ifneq ($(filter hotfix all,$(patch)),)
PATCHES += get_function_proto
PATCHES += check_stack_overflow
PATCHES += infinity_handling
PATCHES += atomic_new_class_id
PATCHES += dynamic_import_sync
endif

ifneq ($(filter msvc all,$(patch)),)
PATCHES += basic_msvc_compat
endif

ifneq ($(filter exports all,$(patch)),)
PATCHES += read_module_exports
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
