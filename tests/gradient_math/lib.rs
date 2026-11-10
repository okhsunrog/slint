// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(test)]

use i_slint_core::graphics::{Color, GradientStop, LinearGradientBrush, linear_gradient_tangent};

fn gradient(angle: f32) -> LinearGradientBrush {
    LinearGradientBrush::new(
        angle,
        [
            GradientStop { color: Color::from_rgb_u8(255, 0, 0), position: 0.0 },
            GradientStop { color: Color::from_rgb_u8(0, 0, 255), position: 1.0 },
        ],
    )
}

fn check(brush: &LinearGradientBrush, rotation: f32) {
    let angle = brush.angle() + rotation;
    #[cfg(feature = "std")]
    let expected = angle.to_radians().tan().abs();
    #[cfg(not(feature = "std"))]
    let expected = num_traits::float::Float::tan(angle.to_radians()).abs();
    let actual = linear_gradient_tangent(brush, rotation);
    if expected.is_nan() {
        assert!(actual.is_nan());
    } else {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "angle={}, rotation={rotation}",
            brush.angle()
        );
    }
}

#[test]
fn constructor_preserves_tangent_and_thread_transfer() {
    #[cfg(feature = "std")]
    let first = std::thread::spawn(|| gradient(90.0)).join().unwrap();
    #[cfg(not(feature = "std"))]
    let first = gradient(90.0);
    check(&first, 0.0);
    let cloned = first.clone();
    check(&cloned, -90.0);
    #[cfg(feature = "std")]
    std::thread::spawn(move || check(&cloned, 180.0)).join().unwrap();

    let rotations = [-270.0, -90.0, 0.0, 90.0, 180.0, 270.0];
    for angle in [
        -0.0,
        0.0,
        0.00001,
        45.0,
        89.99999,
        90.0,
        90.00001,
        180.0,
        270.0,
        360.0,
        -450.0,
        1e20,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        for rotation in rotations {
            check(&gradient(angle), rotation);
        }
    }
    let mut bits = 0x12345678_u32;
    for _ in 0..4096 {
        bits = bits.wrapping_mul(1664525).wrapping_add(1013904223);
        let brush = gradient(f32::from_bits(bits));
        for rotation in rotations {
            check(&brush, rotation);
        }
    }
}
