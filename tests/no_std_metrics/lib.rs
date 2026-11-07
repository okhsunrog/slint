// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#[cfg(test)]
mod tests {
    use slint::ComponentHandle;
    use slint::platform::software_renderer::{
        MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel,
    };
    use slint::platform::{Platform, PlatformError, WindowAdapter};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        time::Duration,
    };

    slint::slint! {
        export component TestWindow inherits Window {
            width: 16px;
            height: 16px;
            background: red;
        }
    }

    struct TestPlatform {
        window: Rc<MinimalSoftwareWindow>,
        time: Rc<Cell<Duration>>,
        logs: Rc<RefCell<Vec<String>>>,
    }

    impl Platform for TestPlatform {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(self.window.clone())
        }
        fn duration_since_start(&self) -> Duration {
            self.time.get()
        }
        fn debug_log(&self, arguments: core::fmt::Arguments) {
            self.logs.borrow_mut().push(arguments.to_string());
        }
    }

    #[test]
    fn embedded_metrics_follow_compile_time_configuration() {
        let options = option_env!("SLINT_DEBUG_PERFORMANCE");
        assert!(options.is_none() || options == Some("refresh_lazy,console"));
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        let time = Rc::new(Cell::new(Duration::ZERO));
        let logs = Rc::new(RefCell::new(Vec::new()));
        slint::platform::set_platform(Box::new(TestPlatform {
            window: window.clone(),
            time: time.clone(),
            logs: logs.clone(),
        }))
        .unwrap();
        let ui = TestWindow::new().unwrap();
        window.set_size(slint::PhysicalSize::new(16, 16));
        ui.show().unwrap();
        let mut pixels = [Rgb565Pixel(0); 16 * 16];
        assert!(window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 16);
        }));
        assert!(pixels.iter().all(|pixel| pixel.0 == 0xf800), "pixels: {:?}", pixels.map(|p| p.0));
        time.set(Duration::from_millis(1000));
        slint::platform::update_timers_and_animations();
        let logs = logs.borrow();
        let reports: Vec<_> =
            logs.iter().filter(|s| s.contains("average frames per second:")).collect();
        assert_eq!(reports.len(), usize::from(options.is_some()), "{logs:?}");
        if options.is_some() {
            assert!(reports[0].contains("details from last frame:"), "{logs:?}");
        }
    }
}
