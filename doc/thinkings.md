## 几个核心问题

1. 目前 ArceOS 已经实现了“线程”组件 axtask，如何利用它转变为进程？
- 独立的地址空间
2. 如何保持 unikernel 的特性？
- 仅保留单一特权级
3. 划分地址空间后，谁来处理缺页、页无效异常？应用运行在 S 态，一旦触发缺页，将陷入 M 态。
- 应用 alloc 的时候要把页面映射做好

## ArceOS 作为动态链接库

### 需要实现的功能

1. 动态链接的原理：
    - [x] `plt` 和 `got` 如何配合工作：
        1. `.rela.plt`：存储了加载器在动态链接时需要填写的 `.got` 表项，以及动态链接函数的**符号信息**，用于索引 `.dynsym`中的一项：
        ```c
        #define ELF64_R_SYM(i) ((i) >> 32)
        ELF64_R_SYM(Elf64_Rela.r_info)
        ```
        2. `.dynsym`：可以查找到和动态链接符号的相关属性，以及**符号名称**（位于 `.dynstr`）
    - [ ] ABI 是如何保持的
2. ArceOS 需要能加载 ELF
3. 加载时如何填写 .got
4. ArceOS 的编译脚本需要修改，
    - [ ] 编译 axlibc，导出符号表
    - [ ] 自身能够读取 libc 的符号表
    - [ ] 最好能直接将 libc 映射到一个地址段，所有进程共享

### 实现路径

1. `mmap()`：难度较大，因为目前没有进程，也就没有其对应的地址空间
2. ELF 解析
3. 动态链接

## 11 月 23 日

1. ArceOS 的 axstd 和 axlibc 不能同时链接

    - 如果强制链接的话会出现重复的符号

    ```diff
    diff --git a/apps/loader/Cargo.toml b/apps/loader/Cargo.toml
    index ec40139..cbd5a5a 100644
    --- a/apps/loader/Cargo.toml
    +++ b/apps/loader/Cargo.toml
    @@ -5,6 +5,9 @@ edition = "2021"
    
    # See more keys and their definitions at https://doc.rust-lang.org/cargo/reference/manifest.html
    
    +[lib]
    +crate-type = ["staticlib"]
    +
    [dependencies]
    axstd = { path = "../../ulib/axstd", optional = true }
    axconfig = { path = "../../modules/axconfig" }
    diff --git a/scripts/make/build.mk b/scripts/make/build.mk
    index 5c815f5..333fa59 100644
    --- a/scripts/make/build.mk
    +++ b/scripts/make/build.mk
    @@ -2,6 +2,7 @@
    
    include scripts/make/cargo.mk
    include scripts/make/features.mk
    +include scripts/make/build_c.mk
    
    ifeq ($(APP_TYPE), c)
    include scripts/make/build_c.mk
    @@ -31,11 +32,13 @@ else ifeq ($(filter $(MAKECMDGOALS),clippy unittest unittest_no_fail_fast),) # n
    endif
    endif
    
    -_cargo_build:
    +_cargo_build: $(c_lib)
        @printf "    $(GREEN_C)Building$(END_C) App: $(APP_NAME), Arch: $(ARCH), Platform: $(PLATFORM_NAME), App type: $(APP_TYPE)\n"
    ifeq ($(APP_TYPE), rust)
        $(call cargo_build,--manifest-path $(APP)/Cargo.toml,$(AX_FEAT) $(LIB_FEAT) $(APP_FEAT))
    -	@cp $(rust_elf) $(OUT_ELF)
    +	$(call cargo_build,-p axlibc, $(LIB_FEAT))
    +	$(call run_cmd,$(LD),$(LDFLAGS) $< $(rust_lib) target/riscv64gc-unknown-none-elf/release/libarceos_loader.a -o $(OUT_ELF))
    +# @cp $(rust_elf) $(OUT_ELF)
    else ifeq ($(APP_TYPE), c)
        $(call cargo_build,-p axlibc,$(AX_FEAT) $(LIB_FEAT))
    endif
    diff --git a/scripts/make/build_c.mk b/scripts/make/build_c.mk
    index cb92d16..1774c5a 100644
    --- a/scripts/make/build_c.mk
    +++ b/scripts/make/build_c.mk
    @@ -59,18 +59,18 @@ $(obj_dir)/%.o: $(src_dir)/%.c $(last_cflags)
    $(c_lib): $(obj_dir) _check_need_rebuild $(ulib_obj)
        $(call run_cmd,$(AR),rcs $@ $(ulib_obj))
    
    -app-objs := main.o
    +# app-objs := main.o
    
    --include $(APP)/axbuild.mk  # override `app-objs`
    +# -include $(APP)/axbuild.mk  # override `app-objs`
    
    -app-objs := $(addprefix $(APP)/,$(app-objs))
    +# app-objs := $(addprefix $(APP)/,$(app-objs))
    
    -$(APP)/%.o: $(APP)/%.c $(ulib_hdr)
    -	$(call run_cmd,$(CC),$(CFLAGS) $(APP_CFLAGS) -c -o $@ $<)
    +# $(APP)/%.o: $(APP)/%.c $(ulib_hdr)
    +# 	$(call run_cmd,$(CC),$(CFLAGS) $(APP_CFLAGS) -c -o $@ $<)
    
    -$(OUT_ELF): $(c_lib) $(rust_lib) $(libgcc) $(app-objs)
    -	@printf "    $(CYAN_C)Linking$(END_C) $(OUT_ELF)\n"
    -	$(call run_cmd,$(LD),$(LDFLAGS) $^ -o $@)
    +# $(OUT_ELF): $(c_lib) $(rust_lib) $(libgcc) $(app-objs)
    +# 	@printf "    $(CYAN_C)Linking$(END_C) $(OUT_ELF)\n"
    +# 	$(call run_cmd,$(LD),$(LDFLAGS) $^ -o $@)
    
    $(APP)/axbuild.mk: ;
    ```
    **解决方法**：将 rust_libc 和 app 链接在一起，c_libc 再和 rust 代码链接
    - [x] rust loader 可以调用 libc 的函数
    - [ ] ArceOS 需要实现 `mmap()`

## 11 月 27 日

KISS 原则实现了加载时的动态链接：先不实现 mmap 和 ELF 解析，采用硬编码的方式加载 hello

ELF 解析加载过程：
1. 读取 Program Headers，将其中类型为 `LOAD` 的 segment 从文件中加载到内存。
