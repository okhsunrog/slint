# KM003C Flash Size Investigation

Measured on October 29, 2026, with the unchanged KM003C UI and peripherals.
The focus-tracking change reduces flash section bytes by 82,196 on stable Rust.
The resulting image links with the original flash limit and retains the firmware's panic/recovery handler.

## Revisions and Configuration

- Firmware: `okhsunrog/km003c-firmware`, `slint-ui`, `702ea45d571d399985b8ffaf63df0f88fb1c4480`.
- Slint baseline: crates.io 1.18.1; local source comparison: tag `v1.18.1`, `372cf0ee5577c3dfec309a45e7b778ba4e81b734`.
- Fork master at investigation start: `742203f8b7e197110da1fa458ce996580e802125` (1.19.0); not used as the comparison baseline.
- Firmware lockfile SHA-256: `253fc8f74fc77f95c9ae52d6d84180e870c0b767f1f3ddbf3907e18f3d4d4350`.
- Stable: rustc 1.99.1, `58cf8f9c3`, October 13, 2026.
- Nightly: rustc 1.101.0-nightly, `92ffebece`, October 28, 2026; channel date October 29.
- Target: `thumbv7em-none-eabihf`; default profile: `opt-level="z"`, fat LTO, one codegen unit.
- Target Slint features: `unsafe-single-threaded`, `libm`, `renderer-software`, `compat-1-2`; defaults disabled.

`cargo tree --target thumbv7em-none-eabihf -e normal,features,no-proc-macro` excludes host build tools.
Target code does not enable image decoders, SVG, system fonts, or path rendering.
Compiler-side image/font dependencies appearing in the full tree are not firmware flash usage.

## Flash Accounting

`memory.x` reserves `[0x4000, 0x73ffe)` for FLASH: 458,750 bytes.
The native bootloader below `0x4000` remains untouched.
Metadata occupies 64 bytes at `0x4420`; executable code starts at `0x4460`.
The linker copies `.pd_fast` into SRAMH and `.data` into RAM, so their load images also consume flash.
`.bss`, ELF symbol tables, and debug information do not consume flash.

The original build fails with `.data` overflow of 1,582 bytes and aligned `.gnu.sgstubs` overflow of 1,602 bytes.
Diagnostic builds temporarily extend FLASH to `0xffffe` to obtain ELF files and maps.
Those diagnostic images are not installable firmware.
The optimized builds use the original `0x6fffe` limit.

| Section | Registry Baseline | Local Baseline | Local Fix |
| --- | ---: | ---: | ---: |
| `.vector_table` | 640 | 640 | 640 |
| `.km_metadata` | 64 | 64 | 64 |
| `.text` | 319,076 | 319,076 | 295,556 |
| `.rodata` | 137,412 | 134,932 | 76,256 |
| `.pd_fast` | 1,932 | 1,932 | 1,932 |
| `.data` | 780 | 780 | 780 |
| `.bss` (RAM) | 58,904 | 58,904 | 58,904 |
| Flash linker extent | 460,352 | 457,856 | 375,680 |
| Remaining budget | -1,602 | 894 | 83,070 |

Extent includes gaps and the aligned address of the empty `.gnu.sgstubs` output section, matching LLD's limit check.
The section reduction is 82,196 bytes; the extent reduction is 82,176 bytes because alignment changes.
Source paths embedded in panic locations explain the separate 2,480-byte `.rodata` difference between registry and local builds.
Do not attribute that path difference to the fix.
Static RAM use is unchanged; peak stack, allocator usage, and device timing were not measured.

## Root Cause and Fix

`WindowInner::track_focus_item` downcast every focused item to `TextInput`.
The downcast references `TextInputVTable`, retaining its key handlers, numeric parsing, and Unicode editing support even for this display-only UI.
Move position-tracker registration into the accepted `TextInput::focus_event(FocusIn)` path.
Keep the existing window-owned tracker, weak references, position callback, and focus-loss cleanup.
Read-only inputs still register; disabled inputs return before registration.
No item ABI, renderer option, font, UI feature, or public API changes.

The resulting ELF has no `TextInputVTable`, `UnicodeWordIndices`, or `POWER_OF_FIVE_128` symbols.
The last table alone occupied 10,416 bytes in the baseline.
TextInput removal also drops substantial Unicode data and editing code.
The bitmap font remains embedded: its named generated resource symbols total approximately 18.8 KB in the baseline.

Size-sorted baseline symbols show approximately 87.6 KB of named i-slint-core functions and 24.0 KB of software-renderer functions.
Core formatting/numeric functions contribute roughly 39.5 KB; application, USB/PD, and executor code are also significant.
These symbol groups are approximate attribution, not independent removable costs: LTO, anonymous data, and shared helpers cross crate boundaries.
For example, the application's UI async task is 15,748 bytes and the USB/RPC join poll function is 13,972 bytes.
Large async monomorphizations are not all Slint runtime overhead.

## Double Precision Evidence

Disassembly contains actual `__aeabi_dadd`, `__aeabi_dmul`, `__aeabi_ddiv`, and comparison/conversion calls.
`libm::math::rem_pio2f::rem_pio2f` alone contains 38 double-helper call sites and occupies 2,368 bytes.
The f32 trigonometric kernels use double intermediates for range reduction and accuracy.
Changing every `f64` to `f32` would change numerical behavior.

The generated key-handler callback for `page = 1 - page` also calls double helpers.
`internal/compiler/generator/rust.rs::compile_binary_operator` promotes numeric arithmetic to f64.
Other retained callers include animation iteration checks, spring evaluation, transforms, and `num_rational::Ratio<u32>::from_f64`.
The latter comes from outside Slint's renderer.

These are candidates for separate, semantics-tested improvements.
The demonstrated large saving comes from reachability, without changing arithmetic precision.
Float formatting still exists after the fix, and shared libm helpers remain.

## Compiler Matrix

All rows below use the registry baseline, its unchanged lockfile, and the same diagnostic FLASH extension.
Only the named variable changes; defaults remain `z`, fat LTO, and one codegen unit.
Sizes are bytes of linker extent.

| Variant | Flash Extent | Remaining Budget |
| --- | ---: | ---: |
| Stable baseline | 460,352 | -1,602 |
| Stable `s` | 525,408 | -66,658 |
| Stable `2` | 728,192 | -269,442 |
| Stable `3` | 755,968 | -297,218 |
| Stable thin LTO | 492,128 | -33,378 |
| Stable 16 codegen units | 464,416 | -5,666 |
| Stable `target-cpu=cortex-m4` | 458,976 | -226 |
| Stable shorter source-path remapping | 459,488 | -738 |
| Nightly baseline | 465,632 | -6,882 |
| Nightly `build-std=core,alloc` | 451,648 | 7,102 |
| Nightly build-std + immediate-abort | 387,328 | 71,422 |

Immediate-abort bypasses this firmware's panic handler, retention diagnostics, and return-to-loader path.
It is an experiment, not the recommended configuration.
The old `panic_immediate_abort` build-std feature is rejected by this nightly; use its explicit panic strategy for reproduction.
Normal target panic behavior is already abort, which still invokes the custom panic handler.

With the Slint fix, stable Cortex-M4 produces extent 374,400 (84,350 bytes spare).
Nightly build-std with the fix produces extent 362,656 (96,094 bytes spare), retaining the panic handler.
The stable default fix already provides ample margin and avoids requiring nightly.
No speed claims follow from these size measurements; hardware PD/GoodCRC timing still needs validation after any compiler-profile change.

## Reproduction

Install official Rust 1.99.1 and nightly-2026-10-29 with the target and `llvm-tools`; install nightly `rust-src` for build-std.
From the firmware's `firmware/` directory:

```sh
cargo +1.99.1 build --release --locked
cargo +1.99.1 rustc --release --locked -- -C link-arg=-Map=/tmp/baseline.map
```

The original limit intentionally fails.
To inspect the oversized ELF, change only FLASH LENGTH to `0xffffe` in a disposable checkout, then repeat the second command.
Restore the real limit before validating the fix.

Use Cargo's `[patch.crates-io]` configuration to select this Slint checkout.
Patch Slint's packages appearing in the firmware lockfile, including `slint`, `slint-build`, `slint-macros`, `i-slint-core`, `i-slint-common`,
`i-slint-compiler`, renderer/backend packages, and the `vtable`/`const-field-offset` helper packages.
Preserve all non-Slint dependency versions.
Compare the unmodified tag and fixed source at the same path using the same patched lockfile.

```sh
cargo +1.99.1 rustc --release --config /tmp/local-slint.toml -- -C link-arg=-Map=/tmp/fixed.map
llvm-size -A target/thumbv7em-none-eabihf/release/km003c-firmware
llvm-nm -S --size-sort --demangle target/thumbv7em-none-eabihf/release/km003c-firmware
llvm-objdump -d --demangle target/thumbv7em-none-eabihf/release/km003c-firmware
python /path/to/slint/scripts/mcu_flash_report.py /tmp/baseline.map /tmp/fixed.map
```

For the matrix, set `CARGO_PROFILE_RELEASE_OPT_LEVEL`, `CARGO_PROFILE_RELEASE_LTO`, or `CARGO_PROFILE_RELEASE_CODEGEN_UNITS` individually.
Preserve the target linker flags when supplying `RUSTFLAGS`:

```sh
RUSTFLAGS='-C link-arg=-Tlink.x -C link-arg=-Tmetadata.x -C target-cpu=cortex-m4' cargo +1.99.1 build --release --locked
cargo +nightly-2026-10-29 build --release --locked -Z build-std=core,alloc
```

## Validation

`SLINT_TEST_FILTER=focus cargo test --manifest-path tests/Cargo.toml -p test-driver-rust --no-default-features` passes all 41 selected tests,
including 36 focus tests and the extended position-tracking regression.
The regression covers motion while focused, focus loss, reacquisition, and rejected focus on a disabled input.
Existing tests also cover read-only input, focus switching, visibility, scrolling, and flickable motion.
Rust formatting and `git diff --check` pass.
The modified firmware links on stable with the original FLASH budget, and symbol inspection confirms the unwanted TextInput dependency disappears.
No device flashing, screenshot comparison, C++ test suite, or full repository test suite was performed.
