# KM003C Flash Size Investigation

Measured on October 29, 2026, with the unchanged KM003C UI and peripherals.
The focus-tracking change reduces flash section bytes by 82,196 on stable Rust.
The resulting image links with the original flash limit and retains the firmware's panic/recovery handler.

The follow-up sections below record additional measurements.
With disabled-metrics elimination, fixed-scale font pruning, and exact integer arithmetic, the latest Slint-only flash extent is 347,168 bytes, leaving 111,582 bytes.
Font pruning requires `SLINT_SCALE_FACTOR=1` for this firmware; without a fixed scale, the compiler preserves the fallback size.

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

A future refactor must allow type checks without retaining the full item vtable,
while preserving successful downcasts, pinning, item lifetime safety, and C++ ABI.
That work remains unimplemented and unvalidated.
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
The updated dependency has not been built or measured in this investigation, and Slint's independent libm callers may still retain double helpers.

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
