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
    - [x] ABI 是如何保持的：函数签名保持一致即可
2. ArceOS 需要能加载 ELF
3. 加载时如何填写 .got
4. ArceOS 的编译脚本需要修改，
    - [x] 编译 axlibc，导出符号表
    - [x] 自身能够读取 libc 的符号表
    - [ ] 最好能直接将 libc 映射到一个地址段，所有进程共享

### 实现路径

1. `mmap()`：难度较大，因为目前没有进程，也就没有其对应的地址空间
2. ELF 解析
3. 动态链接

## 11 月 23 日

1. ArceOS 的 axstd 和 axlibc 不能同时链接

    - 如果强制链接的话会出现重复的符号

**解决方法**：将 rust_libc 和 app 链接在一起，c_libc 再和 rust 代码链接
- [x] rust loader 可以调用 libc 的函数
- [ ] ArceOS 需要实现 `mmap()`

## 11 月 27 日

KISS 原则实现了加载时的动态链接：先不实现 mmap 和 ELF 解析，采用硬编码的方式加载 hello

ELF 解析加载过程：
1. 读取 Program Headers（在 host 上用 readelf），将其中类型为 `LOAD` 的 segment 从文件中加载到内存。

## 11 月 28 日

实现了根据 ELF 头获取需要进行动态链接函数的填充地址和填充值，并进行动态链接。

ELF 解析加载过程：
1. 获取 ELF 头，找到 section header table
2. 根据 .shstrtab 找到：
   - .rela.plt：找到外部链接符号
   - .rela.dyn：链接内部符号
   - .dynsym：动态链接符号表
   - .dynstr：动态链接符号名的字符串表
3. 外部动态链接
   - 用 `rela.r_sym()` 索引 `.dynsym`，获取符号项 `dynsym`
   - 根据 `dynsym` 判断符号属性，符合动态链接的用 `dynsym.st_name` 索引 `dynstr` 获取符号名 `func_name`
   - 根据符号名查找 loader 初始化时填写的链接表，获取链接虚地址 `link_vaddr`
   - 将虚地址 `rela.r_offset` 处的 8 字节填写为 `link_vaddr`
4. 内部动态链接
   - `rela.r_addend` 的值即为链接虚地址 `link_vaddr`
   - 将虚地址 `rela.r_offset` 处的 8 字节填写为 `link_vaddr`

## 11 月 29 日

根据 ELF 头读取 segment 信息，将其中属性为 LOAD 的段加载到内存。此时对 kernel 而言，hello app 不具有单独的进程（或者线程）属性，而是与 loader 为一体。

在 loader 的 Cargo.toml 开启 axstd 的 multitask 属性，可以正常编译，但内核 panic 退出，信息为 `current task is uninitialized`，定位到 `axtask::task::CurrentTask::get()` 函数。显然，在初始化时，已经设置了 `current task`，这里为何报错？

经过进一步阅读源码，发现 `current task` 的指针通过 `axhal::cpu::current_task_ptr()` 获取，而 `axtask/multitask` 开启了 `percpu` 功能，也就是说此时的 `current task` 指针是一个 `percpu` 变量。目前，ArceOS 中通过 `gp` 实现 `percpu`，这与 riscv 的 ABI 是不符的。具体体现为 hello app 一开始就重新加载了 `gp`，把内核里维护 `percpu` 的值冲掉了。

因此，为了保持 ABI，我将 `percpu` 改回了使用 `tp` 维护（前一个修改的 commit 将 `tp` 改为 `gp`）。此时在打开 `axstd/multitask` 后可以正常退出。

## 11 月 30 日

- [x] 实现 `axtask::spawn_from_ptr()`，专门用于创建从 ELF 加载任务的 TCB
- [x] 在 riscv 的任务上下文中加入 `satp`，`context_switch()` 时同时切换页表
- [x] 目前实现了动态分配的页表，但未实现进程退出后页的释放
