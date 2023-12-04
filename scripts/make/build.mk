# Main building script

include scripts/make/cargo.mk
include scripts/make/features.mk

ifneq ($(filter $(APP_TYPE),c mix),)
  include scripts/make/build_c.mk
endif

ifneq ($(filter $(APP_TYPE),rust mix),)
  rust_package := $(shell cat $(APP)/Cargo.toml | sed -n 's/^name = "\([a-z0-9A-Z_\-]*\)"/\1/p')
  rust_target_dir := $(CURDIR)/target/$(TARGET)/$(MODE)
  rust_elf := $(rust_target_dir)/$(rust_package)
endif

ifeq ($(APP_TYPE), mix)
	rust_lib := $(rust_target_dir)/lib$(subst -,_,$(rust_package)).a
endif

ifneq ($(filter $(MAKECMDGOALS),doc doc_check_missing),)  # run `cargo doc`
  $(if $(V), $(info RUSTDOCFLAGS: "$(RUSTDOCFLAGS)"))
  export RUSTDOCFLAGS
else ifeq ($(filter $(MAKECMDGOALS),clippy unittest unittest_no_fail_fast),) # not run `cargo test` or `cargo clippy`
  ifneq ($(V),)
    $(info APP: "$(APP)")
    $(info APP_TYPE: "$(APP_TYPE)")
    $(info FEATURES: "$(FEATURES)")
    $(info arceos features: "$(AX_FEAT)")
    $(info lib features: "$(LIB_FEAT)")
    $(info app features: "$(APP_FEAT)")
  endif
  ifeq ($(APP_TYPE), c)
    $(if $(V), $(info CFLAGS: "$(CFLAGS)") $(info LDFLAGS: "$(LDFLAGS)"))
  else
    $(if $(V), $(info RUSTFLAGS: "$(RUSTFLAGS)"))
    export RUSTFLAGS
  endif
endif

_cargo_build:
	@printf "    $(GREEN_C)Building$(END_C) App: $(APP_NAME), Arch: $(ARCH), Platform: $(PLATFORM_NAME), App type: $(APP_TYPE)\n"
ifeq ($(APP_TYPE), rust)
	$(call cargo_build,--manifest-path $(APP)/Cargo.toml,$(AX_FEAT) $(LIB_FEAT) $(APP_FEAT))
	@cp $(rust_elf) $(OUT_ELF)
else ifeq ($(APP_TYPE), c)
	export RUSTFLAGS="--crate-type=staticlib"
	$(call cargo_rustc,-p axlibc --crate-type staticlib,$(AX_FEAT) $(CLIB_FEAT))
else
	make $(c_lib)
	$(call cargo_rustc,--manifest-path $(APP)/Cargo.toml,$(AX_FEAT) $(LIB_FEAT) $(CLIB_FEAT) $(APP_FEAT))
	$(call run_cmd,$(LD),$(LDFLAGS) $(c_lib) $(rust_lib) -o $(OUT_ELF))
endif

$(OUT_DIR):
	$(call run_cmd,mkdir,-p $@)

$(OUT_BIN): _cargo_build $(OUT_ELF)
	$(call run_cmd,$(OBJCOPY),$(OUT_ELF) --strip-all -O binary $@)

.PHONY: _cargo_build
