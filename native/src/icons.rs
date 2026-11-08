//! The icons the app draws, embedded here: gpui-kit's bundled assets carry only
//! part of Lucide, and a missing path paints nothing with no error.

use std::borrow::Cow;

use anyhow::Result;
use gpui_kit::{AssetSource, SharedString};

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[$((concat!("icons/", $name, ".svg"), include_bytes!(concat!("../assets/icons/", $name, ".svg")))),*];
    };
}

icons!(
    "chev",
    "chev-open",
    "chevron-down",
    "diamond",
    "dots",
    "memory",
    "chevron-right",
    "circle-question-mark",
    "ellipsis",
    "history",
    "house",
    "layout-grid",
    "minus",
    "notebook-text",
    "pencil",
    "rectangle-horizontal",
    "rotate-ccw",
    "search",
    "settings",
    "square",
    "x",
);

pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS.iter().find(|(name, _)| *name == path).map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, _: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS.iter().map(|(name, _)| SharedString::from(*name)).collect())
    }
}
