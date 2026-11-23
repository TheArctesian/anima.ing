use serde::Deserialize;
use tauri::{LogicalPosition, LogicalSize, Rect};

use super::WebViewError;

const SIDE_MAX: f64 = 100_000.0;

/// CSS pixels of the main window.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub fn rect(self) -> Result<Rect, WebViewError> {
        let values = [self.x, self.y, self.width, self.height];
        if !values.iter().all(|value| (0.0..=SIDE_MAX).contains(value)) {
            return Err(WebViewError::Bounds);
        }
        Ok(Rect {
            position: LogicalPosition::new(self.x, self.y).into(),
            size: LogicalSize::new(self.width, self.height).into(),
        })
    }
}
