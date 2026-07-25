//! Display detection via `xrandr`.
//!
//! We parse `xrandr --query` output to determine which outputs are connected
//! and, for the active ones, their current resolution.

use std::collections::BTreeMap;
use std::process::Command;

use crate::error::Error;

/// A connected output and its current mode, if one is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    pub name: String,
    /// Active resolution as `"WIDTHxHEIGHT"`, or `None` if connected but unused.
    pub resolution: Option<String>,
}

/// Query connected monitors. Keyed by output name for stable diffing.
pub fn connected_monitors() -> Result<BTreeMap<String, Monitor>, Error> {
    let output = Command::new("xrandr")
        .arg("--query")
        .output()
        .map_err(Error::Io)?;

    if !output.status.success() {
        return Err(Error::Command {
            command: "xrandr --query".to_string(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_xrandr(&text))
}

/// Parse `xrandr --query` output into a map of connected monitors.
///
/// Output lines look like:
/// ```text
/// eDP-1 connected primary 1920x1080+0+0 (normal ...) 344mm x 194mm
///    1920x1080     60.00*+
/// HDMI-1 disconnected (normal ...)
/// ```
/// The active resolution is taken from the connection line's geometry field
/// (`1920x1080+0+0`); a connected output without geometry has no active mode.
fn parse_xrandr(text: &str) -> BTreeMap<String, Monitor> {
    let mut monitors = BTreeMap::new();

    for line in text.lines() {
        // Mode lines are indented; we only care about output header lines.
        if line.starts_with(char::is_whitespace) {
            continue;
        }

        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let Some(status) = fields.next() else { continue };

        if status != "connected" {
            continue;
        }

        let resolution = fields.find_map(parse_geometry);

        monitors.insert(
            name.to_string(),
            Monitor {
                name: name.to_string(),
                resolution,
            },
        );
    }

    monitors
}

/// Extract `"1920x1080"` from a geometry field like `"1920x1080+0+0"`.
/// Returns `None` for fields that are not geometry (e.g. `"primary"`).
fn parse_geometry(field: &str) -> Option<String> {
    let res = field.split('+').next()?;
    let (w, h) = res.split_once('x')?;
    let is_dimension = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    if is_dimension(w) && is_dimension(h) {
        Some(res.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connected_and_ignores_disconnected() {
        let sample = "\
Screen 0: minimum 320 x 200, current 1920 x 1080, maximum 16384 x 16384
eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 344mm x 194mm
   1920x1080     60.00*+  59.97    59.96
HDMI-1 disconnected (normal left inverted right x axis y axis)
DP-1 connected 2560x1440+1920+0 (normal left inverted right x axis y axis) 597mm x 336mm
   2560x1440     59.95*+
";
        let monitors = parse_xrandr(sample);
        assert_eq!(monitors.len(), 2);
        assert_eq!(
            monitors["eDP-1"].resolution.as_deref(),
            Some("1920x1080")
        );
        assert_eq!(monitors["DP-1"].resolution.as_deref(), Some("2560x1440"));
        assert!(!monitors.contains_key("HDMI-1"));
    }

    #[test]
    fn connected_without_active_mode_has_no_resolution() {
        let sample = "DP-2 connected (normal left inverted right x axis y axis)\n";
        let monitors = parse_xrandr(sample);
        assert_eq!(monitors["DP-2"].resolution, None);
    }
}
