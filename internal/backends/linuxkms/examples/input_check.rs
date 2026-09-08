// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Run on an isolated VM console to check input, cursor repainting, and VT cleanup.
//! The window closes after 30 seconds. Use only dummy text for this check.

use slint::ComponentHandle;

slint::slint! {
    export component InputCheck inherits Window {
        background: #243040;
        forward-focus: input;
        VerticalLayout {
            padding: 40px;
            spacing: 20px;
            Text { text: "KMS input check — closes in 30 seconds"; color: white; }
            Rectangle {
                height: 64px;
                background: #405060;
                input := TextInput { color: white; font-size: 24px; }
            }
            Rectangle { background: #607080; }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint::platform::set_platform(Box::new(
        i_slint_backend_linuxkms::BackendBuilder::default()
            .with_renderer_name("skia-software".into())
            .build()?,
    ))?;
    let app = InputCheck::new()?;
    slint::Timer::single_shot(std::time::Duration::from_secs(30), || {
        slint::quit_event_loop().unwrap();
    });
    app.run()?;
    Ok(())
}
