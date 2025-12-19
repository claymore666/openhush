//! Tray icon creation.
//!
//! Creates a simple microphone icon for the system tray.

use tray_icon::Icon;

/// Create the tray icon
///
/// Creates a simple 32x32 microphone icon programmatically.
pub fn create_icon() -> Result<Icon, String> {
    // Create a simple 32x32 RGBA icon
    let size = 32_u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    // Draw a simple microphone shape
    let center_x = size / 2;
    let center_y = size / 2;

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;

            // Distance from center
            let dx = (x as i32 - center_x as i32).abs();
            let dy = y as i32 - center_y as i32;

            // Microphone body (oval shape in upper portion)
            let in_mic_body = dx < 8 && dy > -12 && dy < 4;

            // Microphone stand (vertical line)
            let in_stand = dx < 2 && (4..10).contains(&dy);

            // Microphone base (horizontal line)
            let in_base = (10..12).contains(&dy) && dx < 8;

            // Microphone arc (curved holder)
            let arc_radius = 10;
            let arc_center_y = 6;
            let dist_to_arc = ((dx * dx + (dy - arc_center_y) * (dy - arc_center_y)) as f32).sqrt();
            let in_arc = (dist_to_arc - arc_radius as f32).abs() < 2.0
                && dy > arc_center_y
                && dy < arc_center_y + 6;

            if in_mic_body || in_stand || in_base || in_arc {
                // White color with full opacity
                rgba[idx] = 255; // R
                rgba[idx + 1] = 255; // G
                rgba[idx + 2] = 255; // B
                rgba[idx + 3] = 230; // A (slightly transparent)
            } else {
                // Transparent
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())
}

/// Create a recording indicator icon (red tint)
#[allow(dead_code)]
pub fn create_recording_icon() -> Result<Icon, String> {
    let size = 32_u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let center_x = size / 2;
    let center_y = size / 2;

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;

            let dx = (x as i32 - center_x as i32).abs();
            let dy = y as i32 - center_y as i32;

            let in_mic_body = dx < 8 && dy > -12 && dy < 4;
            let in_stand = dx < 2 && (4..10).contains(&dy);
            let in_base = (10..12).contains(&dy) && dx < 8;

            let arc_radius = 10;
            let arc_center_y = 6;
            let dist_to_arc = ((dx * dx + (dy - arc_center_y) * (dy - arc_center_y)) as f32).sqrt();
            let in_arc = (dist_to_arc - arc_radius as f32).abs() < 2.0
                && dy > arc_center_y
                && dy < arc_center_y + 6;

            if in_mic_body || in_stand || in_base || in_arc {
                // Red color for recording
                rgba[idx] = 255; // R
                rgba[idx + 1] = 80; // G
                rgba[idx + 2] = 80; // B
                rgba[idx + 3] = 255; // A
            } else {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())
}
