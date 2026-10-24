use ksni::{Icon, MenuItem, Tray};
use ksni::menu::{StandardItem, SubMenu, CheckmarkItem};
use crate::config::APP_NAME;

// Decode a PNG bytes slice → ksni Icon (ARGB32, network byte order)
fn png_to_icon(bytes: &[u8]) -> Option<Icon> {
    use png::ColorType::*;
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let w = info.width as i32;
    let h = info.height as i32;
    let raw = &buf[..info.buffer_size()];

    // Convert to ARGB32 network byte order (big-endian ARGB per pixel)
    let argb: Vec<u8> = match info.color_type {
        Rgba => raw.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect(),
        Rgb  => raw.chunks_exact(3).flat_map(|p| [255u8, p[0], p[1], p[2]]).collect(),
        _    => return None,
    };
    Some(Icon { width: w, height: h, data: argb })
}

static ICON_IDLE: std::sync::OnceLock<Option<Icon>> = std::sync::OnceLock::new();
static ICON_REC:  std::sync::OnceLock<Option<Icon>> = std::sync::OnceLock::new();

// Generate a 22x22 filled circle icon. Used for idle state (the PNG is white, invisible on light panels).
fn make_circle_icon(r: u8, g: u8, b: u8) -> Icon {
    let size: i32 = 22;
    let cx = size as f32 / 2.0;
    let radius = cx - 1.5;
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx + 0.5;
            let dy = y as f32 - cx + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();
            // anti-alias at the edge
            let a = ((radius - dist + 1.0).clamp(0.0, 1.0) * 220.0) as u8;
            // ARGB32, alpha-premultiplied: transparent pixels must be [0,0,0,0]
            let scale = a as u32;
            data.extend_from_slice(&[a,
                ((r as u32 * scale) / 255) as u8,
                ((g as u32 * scale) / 255) as u8,
                ((b as u32 * scale) / 255) as u8]);
        }
    }
    Icon { width: size, height: size, data }
}

fn idle_icon() -> &'static Option<Icon> {
    // Try the PNG first; fall back to a generated teal circle if it's invisible.
    ICON_IDLE.get_or_init(|| {
        if let Some(icon) = png_to_icon(include_bytes!("../assets/plume-idle.png")) {
            // Check if the icon has enough non-white visible pixels (>5%)
            let total = icon.data.len() / 4;
            let visible = icon.data.chunks_exact(4)
                .filter(|p| p[0] > 30 && !(p[1] > 220 && p[2] > 220 && p[3] > 220))
                .count();
            if visible * 20 > total {
                return Some(icon);
            }
        }
        // PNG is white/invisible — use a generated teal circle
        Some(make_circle_icon(82, 153, 210))
    })
}
fn rec_icon() -> &'static Option<Icon> {
    ICON_REC.get_or_init(|| Some(make_circle_icon(220, 50, 50)))
}

pub struct WisprTray {
    pub recording: bool,
    pub mode_toggle: bool,
    pub processor_label: String,
}

impl Tray for WisprTray {
    fn title(&self) -> String { APP_NAME.into() }

    fn icon_name(&self) -> String { String::new() }

    fn icon_pixmap(&self) -> Vec<Icon> {
        let icon = if self.recording { rec_icon() } else { idle_icon() };
        icon.as_ref().into_iter().cloned().collect()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: if self.recording { "🎤 Recording…".into() } else { format!("{APP_NAME} — idle") },
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            // Status (disabled label)
            StandardItem {
                label: if self.recording { "● Recording…".into() } else { "○ Idle".into() },
                enabled: false,
                ..Default::default()
            }.into(),
            MenuItem::Separator,

            // Mode toggle
            CheckmarkItem {
                label: "Toggle mode".into(),
                checked: self.mode_toggle,
                activate: Box::new(|this: &mut Self| {
                    this.mode_toggle = !this.mode_toggle;
                    crate::config::set_conf_key("MODE", if this.mode_toggle { "toggle" } else { "hold" });
                }),
                ..Default::default()
            }.into(),

            // Processor submenu
            SubMenu {
                label: format!("Processor: {}", self.processor_label),
                submenu: vec![
                    StandardItem { label: "Local (Qwen)".into(),
                        activate: Box::new(|_: &mut Self| crate::config::set_conf_key("PROCESSOR", "local")),
                        ..Default::default() }.into(),
                    StandardItem { label: "Ollama".into(),
                        activate: Box::new(|_: &mut Self| crate::config::set_conf_key("PROCESSOR", "ollama")),
                        ..Default::default() }.into(),
                    StandardItem { label: "OpenAI".into(),
                        activate: Box::new(|_: &mut Self| crate::config::set_conf_key("PROCESSOR", "openai")),
                        ..Default::default() }.into(),
                    StandardItem { label: "None".into(),
                        activate: Box::new(|_: &mut Self| crate::config::set_conf_key("PROCESSOR", "none")),
                        ..Default::default() }.into(),
                ],
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            // Settings window
            StandardItem {
                label: "Settings…".into(),
                icon_name: "preferences-system".into(),
                activate: Box::new(|_: &mut Self| {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).arg("--settings").spawn();
                    }
                }),
                ..Default::default()
            }.into(),

            // Open scratchpad
            StandardItem {
                label: "Open scratchpad…".into(),
                icon_name: "document-edit".into(),
                activate: Box::new(move |_: &mut Self| {
                    let path = crate::config::data_dir().join("scratch.txt");
                    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
                }),
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            // Reload (restart service)
            StandardItem {
                label: "Reload config".into(),
                icon_name: "view-refresh".into(),
                activate: Box::new(|_: &mut Self| crate::restart::request_restart()),
                ..Default::default()
            }.into(),

            // Quit
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_: &mut Self| std::process::exit(0)),
                ..Default::default()
            }.into(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_png_icons_decode_and_have_no_transparent_pixel() {
        for (name, bytes) in [("idle", &include_bytes!("../assets/plume-idle.png")[..]), ("recording", &include_bytes!("../assets/plume-recording.png")[..])] {
            let icon = png_to_icon(bytes).unwrap_or_else(|| panic!("{name} ne se décode pas"));
            assert_eq!((icon.width, icon.height), (256, 256), "{name}");
            // `data` is ARGB: the alpha byte is the first of each pixel.
            assert!(icon.data.chunks_exact(4).all(|px| px[0] == 255), "{name} contient des pixels transparents");
        }
    }

    #[test]
    fn circle_icon_has_correct_dimensions() {
        let icon = make_circle_icon(82, 153, 210);
        assert_eq!(icon.width, 22);
        assert_eq!(icon.height, 22);
        assert_eq!(icon.data.len(), (22 * 22 * 4) as usize);
    }

    #[test]
    fn circle_icon_center_pixel_is_opaque() {
        let icon = make_circle_icon(255, 0, 0);
        // Center pixel: (11, 11) → index (11 * 22 + 11) * 4
        let idx = (11 * 22 + 11) * 4;
        let alpha = icon.data[idx];
        assert!(alpha > 200, "center should be mostly opaque, got alpha={alpha}");
    }

    #[test]
    fn circle_icon_corner_pixel_is_transparent() {
        let icon = make_circle_icon(255, 0, 0);
        // Top-left corner is outside the circle
        let alpha = icon.data[0];
        assert!(alpha < 30, "corner should be transparent, got alpha={alpha}");
    }

    #[test]
    fn circle_icon_color_channels() {
        let icon = make_circle_icon(100, 150, 200);
        let idx = (11 * 22 + 11) * 4; // center pixel, ARGB order
        let a = icon.data[idx] as u32;
        // Values are alpha-premultiplied: channel = (input * alpha) / 255
        assert_eq!(icon.data[idx + 1], (100 * a / 255) as u8); // R
        assert_eq!(icon.data[idx + 2], (150 * a / 255) as u8); // G
        assert_eq!(icon.data[idx + 3], (200 * a / 255) as u8); // B
    }
}
