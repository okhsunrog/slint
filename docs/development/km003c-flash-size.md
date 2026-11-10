# KM003C Flash Size Investigation

Measured on October 29, 2026, with the unchanged KM003C UI and peripherals.
The focus-tracking change reduces flash section bytes by 82,196 on stable Rust.
The resulting image links with the original flash limit and retains the firmware's panic/recovery handler.

The follow-up sections below record additional measurements.
With disabled-metrics elimination, fixed-scale font pruning, and exact integer arithmetic, the earlier Slint-only flash extent was 347,168 bytes, leaving 111,582 bytes.
Font pruning requires `SLINT_SCALE_FACTOR=1` for this firmware; without a fixed scale, the compiler preserves the fallback size.

The latest measured combination occupies 316,416 bytes, leaving 142,334 bytes of flash.
It includes exact integer comparisons, constructor-registered gradient math, the earlier Slint changes, USB PD 3.1.0, the local uom-ng fix, and RGB565 BE.
The external dependency and firmware patches remain local; the ordinary firmware lockfile alone does not reproduce this result.
See the follow-up sections for controlled comparisons and validation limits.

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
The bitmap font remains embedded: its named generated resource symbols total 18,556 bytes in the baseline.

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

## Follow-Up: Disabled Metrics, November 7, 2026

The next pass starts from the published `a9a9a222e3c6c768eb4294e04e8a35e9292ccdd1` branch head.
Firmware `slint-ui` is now `baebbf4a18d4369e8debfb3799ce9c8b734e8a7b`; its changes since `702ea45` only affect documentation and hardware logs.
Rust remains pinned to 1.99.1, and the firmware dependency lockfile before local patches is unchanged.
A fresh build reproduces the previous 375,680-byte extent exactly.

The binary still contained `RenderingMetricsCollector::ensure_reporting` and its timer callback, despite `SLINT_DEBUG_PERFORMANCE` being absent.
`RenderingMetrics::fmt` formats the dirty region, whose floating-point coordinates retain Rust's float-to-decimal implementation.
The collector constructor already returns `None` for this configuration, but that fact does not eliminate every retained call path.

Share the embedded compile-time options between construction and `measure_frame_rendered`.
Return immediately from measurement when the embedded options are absent.
This preserves the constructor's existing contract and makes the unreachable measurement code removable locally.
The `std` implementation is unchanged; explicitly enabled embedded metrics still execute the original reporting path.
No precision changes, panic-handler changes, feature removals, or additional runtime allocations are introduced.

| Metric | Previous Fix | With Metrics Fix |
| --- | ---: | ---: |
| `.text` | 295,556 | 278,840 |
| `.rodata` | 76,256 | 72,584 |
| `.data` | 780 | 764 |
| `.bss` | 58,904 | 58,904 |
| Flash section bytes | 375,228 | 354,824 |
| Flash linker extent | 375,680 | 355,264 |
| Remaining flash budget | 83,070 | 103,486 |

The additional saving is 20,404 section bytes, or 20,416 bytes of linker extent.
Symbol inspection confirms that metrics reporting and `float_to_decimal_common_exact` / `float_to_decimal_common_shortest` disappear.
Double-precision arithmetic helpers remain: numeric formatting and arithmetic are separate costs.

### Regression Test

The dedicated `test-no-std-metrics` package renders a 16-by-16 red RGB565 frame through `MinimalSoftwareWindow`.
It checks every pixel, advances a simulated clock, and captures the platform's diagnostic messages.
Both invocations pass:

```sh
# Run separately to avoid feature unification with desktop test packages.
env -u SLINT_DEBUG_PERFORMANCE cargo test --locked --manifest-path tests/Cargo.toml -p test-no-std-metrics
SLINT_DEBUG_PERFORMANCE=refresh_lazy,console cargo test --locked --manifest-path tests/Cargo.toml -p test-no-std-metrics
```

The first run verifies that no FPS report appears.
The second verifies the FPS report and the last frame's dirty-region details.
The normal dependency feature tree confirms that `slint/std` and `i-slint-core/std` are disabled for this package.
The firmware release build also links under its original flash limit.
Hardware timing, the full repository test suite, and device flashing were not performed.

### Additional Stable LLVM Experiments

These results include the metrics fix and retain the default `z`, fat LTO, and one codegen unit.
Two additional machine-outliner passes reduce extent from 355,264 to 354,912 bytes, saving another 352 bytes.
Five additional passes produce the same size.
Combining two outliner passes with `target-cpu=cortex-m4` produces 354,560 bytes (104,190 bytes spare).
Explicitly enabling LLVM's merge-functions pass produces the same 355,264-byte extent as the default.

The machine-outliner option is an internal LLVM tuning option rather than a stable Rust interface:

```sh
RUSTFLAGS='-C link-arg=-Tlink.x -C link-arg=-Tmetadata.x -C llvm-args=-machine-outliner-reruns=2' \
  cargo +1.99.1 build --release --config /tmp/local-slint.toml
```

Additional outlining can alter call overhead and stack behavior, so it needs device timing checks before adoption in the PD firmware.
The source fix provides nearly all the demonstrated gain without these tuning options.

## Follow-Up: Unused Font Sizes, November 7, 2026

The glyph embedder unconditionally included a 12 px bitmap set.
The KM003C UI specifies 14 px for its window default and text items, so the extra set is unused at scale 1.
The compiler now omits the fallback set only when the scale is fixed and all collected font-size bindings are constant, positive literals.
An unknown, mutable, non-positive, or missing size keeps the fallback.
An explicitly requested 12 px size is always retained.

Fixing the scale is necessary: a runtime scale change can make even a constant logical font size select a smaller bitmap set.
`SLINT_SCALE_FACTOR` already sets the generated window's constant scale factor.
This change adds no new switch and does not remove any characters or alter the retained glyphs.

| Measurement | Previous Compiler, Scale 1 | Font Pruning, Scale 1 |
| --- | ---: | ---: |
| `.text` | 278,888 | 278,888 |
| `.rodata` | 72,584 | 64,552 |
| `.data` | 764 | 764 |
| `.bss` (RAM) | 58,904 | 58,904 |
| Flash linker extent | 355,296 | 347,264 |
| Remaining budget | 103,454 | 111,486 |

Both builds use Rust 1.99.1, the same paths, lockfile, default release profile,
and firmware revision `baebbf4a18d4369e8debfb3799ce9c8b734e8a7b`.
The change removes exactly 8,032 bytes of `.rodata`.
Compared with the preceding 355,264-byte build without fixed scale, the combined reduction is 8,000 bytes.
Fixing the scale adds 48 bytes of code and changes alignment.
Static RAM is unchanged.
Without `SLINT_SCALE_FACTOR`, a verification build remains exactly 355,264 bytes, matching the preceding compiler.

To enable the optimization for the fixed-density KM003C display:

```sh
SLINT_SCALE_FACTOR=1 cargo +1.99.1 rustc --release --locked \
  --config /tmp/local-slint.toml -- -C link-arg=-Map=/tmp/font-pruned.map
```

The compiler regression test covers constant and mutable sizes, unspecified and zero sizes, explicit 12 px, fixed scale 2, and runtime scale changes.
It compares every retained 14 px glyph's bitmap, bearings, dimensions, and advance by Unicode code point against a compilation that retains both sizes.
The Linux test command uses the existing fontconfig dynamic-loading feature:

```sh
cargo test --locked -p i-slint-compiler \
  --features renderer-software,i-slint-common/fontconfig-dlopen \
  --test embedded_font_sizes
```

All six regression tests pass.
The complete compiler library and integration test run also passes: 367 tests, none failed or filtered.

```sh
cargo test --locked -p i-slint-compiler \
  --features renderer-software,rust,i-slint-common/fontconfig-dlopen --lib --tests
```

A repeated final firmware build has the same 347,264-byte extent.
ELFs need not be byte-identical: the embedder assigns glyph indices from unordered character coverage before sorting the character map.
The glyph comparison therefore follows Unicode code points rather than comparing storage indices.

### Further Reachability Work

A disposable experiment removed the generic `Flickable` downcasts and scrolling helper.
On the initial font-pruned build, extent fell from 347,232 to 333,440 bytes.
This is a diagnostic reachability measurement, not a usable optimization: removing those helpers breaks scrolling and focus visibility.
All diagnostic edits were restored before publication.
The final fixed-scale build has different code size, so this delta must not be added to its result as a promised saving.

The later Flickable follow-up below removes these retention paths by registering operations from item initialization.
It preserves the existing downcast mechanism, pinning, item lifetime safety, and C++ vtable layout.
It does not implement a general replacement for vtable-based type identity.
The measured font change does not modify item dispatch or scrolling.
Device rendering, stack usage, and PD timing have not been tested on hardware.

## Follow-Up: Arithmetic and Double Precision

The fixed-scale font-pruned baseline contains 4,078 bytes of identified double helper bodies and ABI wrappers.
This sum deduplicates addresses and includes `__adddf3`, `__muldf3`, and `__divdf3`, not just their small `__aeabi_*` wrappers.
It excludes callers and libm kernels, so it is not the total cost of double precision.

Disassembly of that baseline identifies these direct call sites into double helpers:

| Caller | Call Sites |
| --- | ---: |
| `libm::rem_pio2f` | 44 |
| `Ratio<u32>::from_f64` | 13 |
| `libm::k_tanf` | 10 |
| Animation interpolation | 7 |
| `libm::k_cosf` | 7 |
| `libm::k_sinf` | 5 |
| Generated page key handler | 4 |
| Spring evaluation | 3 |
| Children transform | 3 |

These are static call-site counts, including conversions, not execution counts or exclusive byte costs.
The earlier range-reduction count excluded some conversions.
The spring source already uses `f32::sin_cos` and `f32::exp`.
Replacing explicit `f64` types would therefore miss much of the retained double arithmetic.
The f32 libm kernels use double intermediates for accuracy and range reduction.

### Exact Integer Addition and Subtraction

The Rust generator previously compiled `page = 1 - page` through double conversion, subtraction, and a saturating float-to-i32 cast.
When an addition or subtraction is immediately cast to i32, the generator now recognizes i32 property/parameter operands and exactly representable i32 literals.
It emits `saturating_add` or `saturating_sub` instead.
The full mathematical sum or difference of two i32 values is exact in f64, so saturation preserves the original result.

The optimization deliberately excludes nested arithmetic, fractional or out-of-range literals, multiplication, and division.
For example, saturating `(a + b)` before evaluating `(a + b) - b` would change overflow cancellation.
Changing intermediates to f32 would also lose integer precision above 16,777,216.
Numeric promotion casts are transparent in the existing Rust generator and can be traversed without rounding.

| Measurement | Before | After |
| --- | ---: | ---: |
| Flash extent | 347,264 | 347,168 |
| Remaining budget | 111,486 | 111,582 |
| Double helper calls in page key handler | 4 | 0 |

The section-byte reduction is 104 bytes; alignment makes the extent reduction 96 bytes.
The remaining callers still retain the shared double helpers.
No device timing claim follows from the reduced call count.
The ARM output uses `qsub r1, r4, r0` for the page subtraction.
It replaces calls to `__aeabi_i2d`, `__aeabi_dsub`, `__aeabi_dcmpgt`, and `__aeabi_dcmpun` in that handler.

`tests/cases/expr/integer_arithmetic.slint` compares generated results with the original f64 arithmetic on 121 operand pairs.
It covers i32 endpoints, values beyond f32's exact-integer range, saturation, subtraction order, fractional literals, and cancellation.
The complete compiler library/integration suite passes all 367 tests.
The unfiltered Rust test driver passes 849 tests, with 11 ignored and none failed or filtered out.
Qt tests are excluded by the driver's disabled default features.

```sh
cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-rust --no-default-features
```

The host needs fontconfig linker metadata; this environment used a local pkg-config file pointing to its installed fontconfig runtime library.
A final firmware rebuild with the original registry uom dependency confirms the 347,168-byte extent.

### Double Arithmetic Outside Slint

**Upstream status correction:** the measurements below use the firmware's locked `usbpd` 2.0.0 and `uom` 0.36.0.
They do not describe the latest usbpd release.
[okhsunrog's PR #58](https://github.com/elagil/usbpd/pull/58) migrated usbpd to `uom-ng` 0.39.0 with compile-time conversion factors.
It was merged on October 30, 2026 and included in [usbpd 3.0.0](https://github.com/elagil/usbpd/releases/tag/usbpd/v3.0.0).
The latest GitHub release checked on November 7 is [3.1.0](https://github.com/elagil/usbpd/releases/tag/usbpd/v3.1.0).
Applications exchanging quantities with usbpd must also migrate their `uom` dependency to the `uom-ng` package under the `uom` alias.
The existing Cargo requirement `usbpd = "2.0.0"` does not select 3.x through a routine lockfile update.

Prefer evaluating that upstream migration before extending the historical local uom experiment below.
The PR reports elimination of double and num-rational symbols in its USB PD example; that is not a measurement of this Slint firmware.
The subsequent migration measurement below tests the updated dependency in this firmware.
Slint's independent libm callers can still retain double helpers.

The callers of `Ratio<u32>::from_f64` resolve to USB PD policy-engine and power-supply conversion functions.
For example, `usbpd` 2.0.0 `FixedSupply::voltage` constructs an `ElectricPotential` with `_50millivolts`.
The `uom` 0.36.0 `unit!` macro implements integer-storage conversion coefficients and additive constants via `Ratio::from_f64`.
This includes conversion constants equal to zero and one.
This path is separate from Slint and must not be counted as renderer overhead.

A separate local `uom` experiment short-circuits only zero and one in the `PrimInt, BigInt` conversion helper:

```rust
if value == 0.0 {
    return T::from_integer(<V as $crate::num::Zero>::zero());
}
if value == 1.0 {
    return T::from_integer(<V as $crate::num::One>::one());
}
```

All other values still use the original `FromPrimitive::from_f64` implementation.
A host test confirms equality with the original conversion for the firmware's `Ratio<u32>` storage.
This is an external dependency experiment, not a committed Slint change or hardware-validated firmware patch.

| Identical Local uom Path | Original | Zero/One Fast Path |
| --- | ---: | ---: |
| `.text` | 278,776 | 276,912 |
| `.rodata` | 64,112 | 63,600 |
| `.data` | 764 | 764 |
| `.bss` | 58,904 | 58,904 |
| Flash extent | 346,720 | 344,352 |

The controlled extent saving is 2,368 bytes; section bytes decrease by 2,376.
Comparing registry uom against the modified local source would overstate the saving by 448 bytes because source paths differ.
The firmware lockfile was restored after the experiment; no firmware or external dependency changes were pushed.

The larger remaining opportunities are eliminating unused math callers and specializing known rational unit coefficients.
Replacing f64 with f32 globally, or substituting approximate trig functions, requires a different numerical contract and explicit error bounds.
It is not an equivalent size optimization.

## Firmware Migration to usbpd 3.1.0

The local firmware now uses the GitHub 3.1.0 release at `b1f0bb9720e362039ac5e5a511199c828da26c04`.
The registry index available during this run contains 3.0.0 but not 3.1.0, so both usbpd and usbpd-traits use that pinned Git revision.
The direct units dependency is `uom = { package = "uom-ng", version = "0.39.0", default-features = false, features = ["si", "u32"] }`.
The adapter implements the new `SinkDpm`, `DrpDevicePolicyManager`, and `EprDevicePolicyManager` traits with their defaults.
The existing voltage requests, 100 mA request, and PHY callbacks are unchanged; the library's default EPR feature stays enabled.
No firmware changes were pushed.

With this Slint branch, fixed scale 1, and the same release profile, the image links:

| Measurement | usbpd 2.0.0 | usbpd 3.1.0 |
| --- | ---: | ---: |
| `.text` | 278,784 | 284,576 |
| `.rodata` | 64,552 | 65,068 |
| `.data` | 764 | 764 |
| `.bss` | 58,904 | 59,896 |
| Flash extent | 347,168 | 353,472 |
| Remaining budget | 111,582 | 105,278 |

This complete version upgrade increases extent by 6,304 bytes and static RAM by 992 bytes.
It is not an isolated comparison of unit conversion implementations.
The new ELF still contains `Ratio<u32>::from_f64`, with seven direct calls from the USB PD policy engine and fixed-request construction.
Thus, the symbol-elimination result from the upstream PR's example does not reproduce in this firmware and profile.
The legacy uom package is absent from the dependency graph; both firmware and usbpd use the same uom-ng package.

A separate build with registry Slint 1.18.1 and the firmware's default scale still fails the original flash limit by 7,874 bytes.
The ordinary firmware lockfile retains registry Slint; the successful measurement uses the local Slint patch configuration described above.
The migration patch contains only the firmware manifest, lockfile, and PD adapter changes.
Device negotiation, GoodCRC timing, and recovery behavior require hardware validation after this stack upgrade.

### Root Cause of the Size Increase and Retained Double Conversion

The migration's size increase has two distinct causes: new protocol behavior and incomplete compile-time evaluation in uom-ng.
Named symbol differences locate the main contributors before fixing uom-ng:

| Symbol | usbpd 2.0.0 | usbpd 3.1.0 | Increase |
| --- | ---: | ---: | ---: |
| Sink `update_state` future poll | 7,952 | 11,096 | 3,144 |
| `uom::fraction::Fraction::recip` | 0 | 1,800 | 1,800 |
| `wait_for_good_crc` future poll | 476 | 1,160 | 684 |
| `receive_message_inner` future poll | 1,624 | 1,868 | 244 |
| PD task static pool (RAM) | 2,192 | 3,184 | 992 |

These symbols do not partition the entire binary; LTO and outlining share code among functions.
The PD task pool accounts for the entire measured static RAM increase.
Version 3.1.0 adds a two-message receive queue and acknowledges incoming messages while awaiting GoodCRC.
Version 2.0.0 instead returns an error for an unexpected message in that path.
The new queue and async control flow are functional changes, not unused UI code to remove.

In uom-ng 0.39.0, `system.rs::from_base` evaluates `conversion_factor` in a `const` block but calls `factor.recip()` outside it.
Being declared `const fn` does not force a call to execute at compile time.
In this firmware's `z` profile, the reciprocal remains a runtime operation on a 128-bit fraction.
The optimizer also retains the fallback coefficient conversions through `Ratio::from_f64`.

A standalone no_std ARM executable reproduces the profile difference with unmodified uom-ng.
It converts a black-box u32 voltage to millivolts, uses fat LTO and one codegen unit, and is not flashed to hardware.
With `opt-level="z"`, llvm-size reports 4,250 text bytes and retains `Ratio::from_f64`.
With `opt-level=3`, it reports 52 text bytes and has no rational-conversion symbols.
These are minimal-executable sizes, not firmware savings.
This explains why the upstream PR's `3` profile can eliminate the conversion while this firmware's `z` profile retains it.
With the constant-reciprocal fix, the same probe at `z` shrinks to 40 text bytes and also has no rational-conversion symbols.

The local fix moves the reciprocal into the same constant evaluation:

```rust
if let Some(factor) = const {
    match conversion_factor::<D, U, V, N>() {
        Some(factor) => factor.recip(),
        None => None,
    }
} {
    if let Some(v) = V::T::scale(v, factor.numer, factor.denom) {
        return v;
    }
}
```

The original runtime fallback below this block is unchanged.
No float precision, unit conversion algorithm, feature, or PD behavior is changed.

| Same Local uom-ng Path | Original 0.39.0 | Constant Reciprocal |
| --- | ---: | ---: |
| `.text` | 284,608 | 279,192 |
| `.rodata` | 64,660 | 63,220 |
| `.data` | 764 | 764 |
| `.bss` | 59,896 | 59,896 |
| Flash extent | 353,088 | 346,240 |
| Remaining budget | 105,662 | 112,510 |

The controlled saving is 6,848 bytes of extent, or 6,856 section bytes.
The 384-byte difference between the registry and local uom-ng baseline must not be attributed to the fix.
The fixed ELF has no `num_rational` symbols and no `Fraction::recip` symbol.
The remaining double helpers are retained by independent Slint/libm callers.
The PD task pool remains 3,184 bytes, preserving the new protocol behavior.

Boundary tests for volt/millivolt conversions pass, including truncation, values above f32's exact-integer range, and the largest non-overflowing u32 inputs.
The uom-ng `si,u32` library suite is not green: both unmodified and patched 0.39.0 pass 178 tests and fail the same 116 tests.
The baseline was built independently; there are no newly failing test names.
These existing integer-unit test failures must not be reported as a passing full suite.
The minimal patch and reproduction sources are retained separately; no changes to an external uom-ng repository were published.
The firmware's ordinary lockfile was restored after the measurement; using the fix requires selecting the patched uom-ng source.

## Native RGB565 Big-Endian Display Path

The local firmware now uses Slint's existing `Rgb565BigEndianPixel` as its line-buffer target.
Slint renders directly into the eight-row tile, removing a 240-pixel intermediate scanline and its copy loop.
The display transport keeps the existing rotation mapping and sends each pixel's `.0.to_ne_bytes()`.
The BE pixel already stores bytes in panel order; applying `to_be_bytes()` again would reverse them on this little-endian MCU.

These measurements use the same optimized Slint, USB PD 3.1.0, constant-reciprocal uom-ng patch, and `SLINT_SCALE_FACTOR=1` as the preceding experiment.

| Firmware Display Path | Native RGB565 With Scanline Copy | Direct RGB565 BE Tile |
| --- | ---: | ---: |
| `.text` | 279,192 | 279,056 |
| `.rodata` | 63,220 | 63,188 |
| `.data` | 764 | 764 |
| `.bss` | 59,896 | 59,416 |
| Flash extent | 346,240 | 346,080 |
| Remaining budget | 112,510 | 112,670 |

The combined change saves 160 bytes of flash extent and 480 bytes of static RAM.
The RAM saving comes from removing the scanline, not from BE pixels occupying less memory.
Both pixel types occupy two bytes.
These measurements do not isolate the format change from removing the copy.

The ARM release build passes.
All eight software-renderer unit tests pass, including RGB565 BE conversion and blending.
All 29 driver tests pass, including the asymmetric-tile test for all four rotations.
No device timing or visual validation was performed, and the change does not add TE synchronization.
The firmware patch remains local; only this report was published to the Slint fork.

## Flickable Operations Registered During Initialization

Generic focus traversal previously referenced the Flickable vtable in two places:
`is_visible_or_clipped_by_flickable` and `try_scroll_into_visible`.
Those references retained the complete item implementation even when the application had no Flickable.

`Flickable::init` now registers two function pointers for type checking and revealing a descendant.
The generic traversal reads these operations from thread-local storage, without referencing a concrete item vtable.
The no_std build uses Slint's existing single-threaded storage implementation.
The final firmware ELF contains neither `FlickableVTable` nor Flickable implementation symbols.
Before this change, the 48-byte vtable retained a 1,440-byte input-event entry point and a 788-byte input-filter entry point, among other code.
The shared double-precision helpers remain; this change does not eliminate all f64 arithmetic.

Registration precedes user bindings and initialization code in the Rust generator, C++ generator, and interpreter.
Before registration, the type query returns false and reveal requests do nothing; the thread has no initialized Flickable yet.
The functions hold no item references, so they remain usable after an individual item tree is destroyed.
Every call still uses the existing checked downcast before accessing Flickable properties.
No new unsafe code, vtable fields, C++ ABI changes, or alternative type identifiers are introduced.
The scrolling algorithm and ancestor traversal are unchanged.
This is a focused registration refactor, not a general redesign of `vtable`.

Measurements use the preceding RGB565 BE firmware, the same USB PD and uom-ng patch, and the same stable compiler and release configuration.

| Measurement | Before | After |
| --- | ---: | ---: |
| `.text` | 279,056 | 266,280 |
| `.rodata` | 63,188 | 62,436 |
| `.data` | 764 | 764 |
| `.bss` | 59,416 | 59,424 |
| Flash extent | 346,080 | 332,544 |
| Remaining budget | 112,670 | 126,206 |

The measured saving is 13,536 bytes of flash extent, at a cost of eight bytes of static RAM.
Applications that use Flickable still retain its implementation and perform indirect calls through the registered functions during focus traversal.
No device timing improvement is claimed.

The ARM release build passes with the original flash limit.
The unfiltered Rust test driver passes 850 tests, with 11 ignored and none failed.
All 22 C++ cases selected by `SLINT_TEST_FILTER=flickable` pass.
These cover nested scrolling, clipped focus navigation, pointer input, and scroll animations.
The new lifecycle test exercises focus before the first Flickable, focus from user initialization, and repeated creation and destruction.
Its Rust weak-reference assertion confirms that registration does not retain the destroyed item tree.
No hardware validation was performed.

Run the test drivers sequentially because they share generated files and build artifacts:

```sh
cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-rust --no-default-features
SLINT_TEST_FILTER=flickable cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-cpp --no-default-features
```

The recorded host runs disable incremental compilation and test debug information to limit disk use.
The C++ run uses the installed fontconfig runtime through local pkg-config metadata, as in the earlier Rust validation.

## Transform and Menu Shortcuts, November 9, 2026

Two independent retention paths remain after the Flickable change.
`ItemRc::children_transform` and `render_item_children` reference the Transform vtable from generic coordinate mapping and clipping checks.
The firmware has no Slint transform properties; its physical display rotation is implemented separately in the display driver.

`Transform::init` now registers the existing matrix calculation and checked type query.
Both generic callers use those registered functions.
The matrix arithmetic, inverse handling, property reads, and clipping exception are unchanged.
The vtable layout and checked downcast implementation are unchanged, with no new unsafe operations.
The manually constructed unit-test tree now calls `register_item_tree`, following the initialization protocol used by generated trees.

The keyboard path separately calls `process_menubar_shortcuts` even when no menubar has been installed.
Its ambiguous-shortcut warning retains `Keys::Display`, which occupies 3,000 bytes in the preceding build.
`setup_menubar_shortcuts` now installs that handler in the window, and keyboard dispatch calls it only when installed.
Shortcut activation, dynamic enabled/visible entries, event consumption, and the ambiguous-shortcut warning are preserved.
The handler holds no menu or item reference; the existing weak menu reference still controls its lifetime.
The final firmware retains neither `Keys::Display` nor the menu-shortcut handler, while `Keys::Debug` remains.

The following builds use the same stable compiler, release options, fixed scale, BE firmware, and patched uom-ng source.

| Measurement | Flickable Baseline | Plus Transform Registration | Plus Menu Registration |
| --- | ---: | ---: | ---: |
| `.text` | 266,280 | 264,640 | 260,248 |
| `.rodata` | 62,436 | 62,388 | 60,052 |
| `.data` | 764 | 764 | 764 |
| `.bss` | 59,424 | 59,432 | 59,432 |
| Flash extent | 332,544 | 330,848 | 324,128 |
| Remaining budget | 126,206 | 127,902 | 134,622 |

Transform registration saves 1,696 bytes of flash extent and adds eight bytes of static RAM.
Menu registration saves a further 6,720 bytes, for a combined saving of 8,416 bytes.
The window gains one optional function-pointer field.
In this ARM build, its enclosing `Rc<MinimalSoftwareWindow>` allocation still requests 688 bytes with eight-byte alignment before and after the menu change.
That layout observation does not guarantee an unchanged allocation size on other targets or configurations.
No device timing or peak-heap measurements were performed.

### Remaining Double Arithmetic

The Transform change removes `TransformVTable`, `libm::k_sinf` (168 bytes), and `libm::k_cosf` (192 bytes) from the firmware.
Those removed symbols are part of the measured saving above, not additional savings.

Disassembly still shows this chain in the software renderer:

```text
PrepareScene::process_rectangle
  -> process_rectangle_impl: linear-gradient angle.to_radians().tan()
     -> libm::rem_pio2f (2,384 bytes)
     -> libm::k_tanf (296 bytes)
        -> double arithmetic and conversion helpers
```

The source expression uses f32, but libm uses double intermediates for range reduction and tangent evaluation.
The common rectangle renderer handles all brush variants, retaining that branch even though this UI uses solid colors.
This patch preserves gradient support and numerical behavior; it does not substitute an approximate tangent or change angle normalization.

Following the LLVM outlined helpers also identifies double conversions, additions, and comparisons in generated UI binding and geometry functions.
Removing the gradient chain alone would therefore not prove that every double helper becomes removable.
These are identified static call paths, not runtime frequency measurements or an exhaustive whole-program call graph.
The current ELF has no spring/sine/cosine implementation symbols; earlier arithmetic measurements described older builds with different retained callers.

### Validation

All 340 core library tests pass, including the new transform registration and inverse test.
It covers the absence of transforms before registration, a 90-degree rotation about a nonzero origin, nonuniform scale, singular inversion, and item-tree release.
Existing clipping and coordinate mapping tests continue to pass.
The unfiltered Rust driver passes 850 tests, with 11 ignored and none failed.
Seven selected C++ tests pass: two transform cases, two rotation cases, two scale/rotate gesture cases, and the menu-shortcut case.
The extended menu test verifies normal key delivery to a separate window without a menu, alongside dynamic and disabled menu shortcuts.
The ARM release build passes with the original memory layout and flash limit.
Device rendering and timing remain untested.

```sh
cargo test --locked -p i-slint-core --lib --features i-slint-common/fontconfig-dlopen
cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-rust --no-default-features
for filter in transform rotation scalerotate menubar_shortcut; do
    cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-cpp --no-default-features -- "$filter"
done
```

These driver runs were sequential, with no overlapping rebuilds of shared test artifacts.

## Exact Comparisons and Gradient Math, November 9, 2026

This follow-up starts from `b75fda60f43249afc2775185e7ebdb51fefee255`.
It uses firmware `baebbf4a18d4369e8debfb3799ce9c8b734e8a7b` with the local USB PD, uom-ng, and RGB565 BE changes described above.
The compiler, optimization settings, UI, font scale, memory layout, and source paths are identical across these three builds.

| Measurement | Published Baseline | Plus Integer Comparisons | Plus Gradient Registration |
| --- | ---: | ---: | ---: |
| `.text` | 260,248 | 259,848 | 252,808 |
| `.rodata` | 60,052 | 60,052 | 59,776 |
| `.data` | 764 | 764 | 764 |
| `.bss` | 59,432 | 59,432 | 59,436 |
| Flash extent | 324,128 | 323,744 | 316,416 |
| Remaining budget | 134,622 | 135,006 | 142,334 |

Integer comparisons save 384 bytes of flash extent.
Gradient registration saves another 7,328 bytes, for a combined saving of 7,712 bytes.
Static RAM increases by four bytes in this firmware.
No device timing or peak-stack measurements were performed.

### Integer Comparison Semantics

The Rust generator previously converted numeric comparisons to f64, including comparisons between two i32 properties.
Numeric equality uses euclid's absolute epsilon of `1e-6`.
Every i32 value and the difference between two i32 values are exactly representable in f64.
Distinct integers differ by at least one, so integer equality and ordering preserve the previous results.

The optimization accepts only int properties, int function parameters, and integral literals within the i32 range.
It also follows representation-preserving numeric casts already emitted unchanged by the generator.
Nested arithmetic, fractional operands, out-of-range literals, and other expression kinds keep the previous path.
The iterative traversal of long binary-expression chains remains intact.
Generated comparisons retain enclosing parentheses, including when return lowering introduces logical negation.

The regression case covers 169 pairs of boundary values, values around the f32 precision boundary, all six comparisons, negation, and chained conditions.
It also checks function parameters, cancellation, mixed arithmetic, fractional epsilon comparisons, infinities, and NaN.
The existing conditional-return regression detected a missing-parentheses error during development; the corrected implementation passes that case.

### Gradient Construction and Compatibility

The common rectangle renderer retains `angle.to_radians().tan().abs()` even when an application uses only solid brushes.
That f32 expression retains libm's double-precision range reduction and tangent intermediates.

Rust's `LinearGradientBrush::new` now registers the unchanged tangent function.
The renderer obtains it through an internal helper that takes a valid gradient and the renderer rotation.
The angle addition, radians conversion, tangent, absolute value, and subsequent rasterization are unchanged.
No angle normalization or approximate arithmetic is introduced.

With `std`, a process-wide `OnceLock` supports constructing a brush on one thread and rendering it on another.
Without `std`, the existing `unsafe-single-threaded` storage mechanism follows Slint's single-core, non-interrupt API contract.
The registry stores only a function pointer and retains no brush allocation.
Registered gradient evaluation adds an indirect call; its device timing has not been measured.
Safe Rust construction uses `new`; cloning preserves that initialization invariant.

C++ constructs the shared-vector gradient representation directly in its header.
The `ffi` feature therefore retains a direct tangent call, without requiring registration or changing the C++ ABI.
Brush layout, gradient stops, and public language syntax are unchanged.
The helper is hidden from generated documentation and belongs to the internal core crate.

### Double-Precision Result

After integer comparisons alone, the identified generated-UI double call paths disappear, while the renderer's tangent path remains.
After gradient registration, the final ELF has no double arithmetic/comparison/conversion helper symbols and no `rem_pio2f` or `k_tanf`.
Disassembly also has no calls to the double helpers previously identified in this investigation.
This result applies to this firmware and configuration; applications using gradients or other double arithmetic can retain those functions.
Integer 64-bit division remains for legitimate firmware work and is not double-precision arithmetic.

The renderer still retains other brush handling, texture/text rendering, rounded rectangles, and f32 math such as `atanf` and `sqrtf`.
Those are separate opportunities requiring their own measurements and behavior checks.
The present change does not disable gradients, fonts, clipping, diagnostics, or UI features.

### Validation and Reproduction

The gradient regression compares the new helper with the previous expression for 17 special values and 4,096 deterministic f32 bit patterns.
Each angle is tested with six renderer rotations, comparing bits for non-NaN results and preserving NaN results.
It also checks cloning and cross-thread construction/use when `std` is enabled.

All four gradient-math configurations pass: `std`, `no_std`, and each with `ffi`.
The core library passes 340 tests, and compiler library/integration tests pass 367.
The unfiltered Rust driver passes 851 tests, with 11 ignored and none failed.
Ten software-renderer screenshot tests pass against existing references, covering five gradient cases with runtime and embedded resources.
The embedded-resource screenshot checks include renderer rotation and partial-rendering comparisons.
No reference image or threshold was changed.
Three selected C++ gradient tests pass, exercising the direct-construction FFI path.
All test drivers ran sequentially through completion.

The dedicated `test-gradient-math` package isolates `no_std` validation from the core crate's development dependencies, which enable `std` through Slint.
Run each feature configuration separately; combining workspace packages can unify features and invalidate a `no_std` check.

```sh
cargo test --locked --manifest-path tests/Cargo.toml -p test-gradient-math
cargo test --locked --manifest-path tests/Cargo.toml -p test-gradient-math --no-default-features
cargo test --locked --manifest-path tests/Cargo.toml -p test-gradient-math --no-default-features --features ffi
cargo test --locked --manifest-path tests/Cargo.toml -p test-gradient-math --features ffi
cargo test --locked -p i-slint-core --lib --features i-slint-common/fontconfig-dlopen
cargo test --locked -p i-slint-compiler --features renderer-software,rust,i-slint-common/fontconfig-dlopen --lib --tests
cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-rust --no-default-features
SLINT_TEST_FILTER=gradient cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-screenshots --no-default-features --features software-embed-assets
cargo test --locked --manifest-path tests/Cargo.toml -p test-driver-cpp --no-default-features -- gradient
```

The measurement artifacts are `integer-comparisons-final.{map,elf}` and `integer-gradient-final.{map,elf,asm,symbols}`.
The map comparison is `integer-gradient-comparison.json`, generated by `scripts/mcu_flash_report.py`.
An isolated prototype used a longer source path and had a different absolute rodata size.
Its controlled saving was also 7,328 bytes; the table above uses only the main checkout to avoid mixing path effects.
