# Embedded Rendering Metrics Test

Run this package separately so desktop packages don't enable `slint/std` through feature unification.
The test uses a host test harness with Slint's embedded runtime configuration.

```sh
env -u SLINT_DEBUG_PERFORMANCE cargo test --locked --manifest-path tests/Cargo.toml -p test-no-std-metrics
SLINT_DEBUG_PERFORMANCE=refresh_lazy,console cargo test --locked --manifest-path tests/Cargo.toml -p test-no-std-metrics
```

The two builds verify identical RGB565 pixels with metrics disabled and enabled.
The simulated timer must produce an FPS report with frame details only in the enabled build.
