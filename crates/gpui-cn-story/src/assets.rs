//! The gallery's asset source: gpui-cn's, plus the Lucide icons only the
//! stories draw.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

gpui_kit::assets::icon_assets!(StoryExtras, [Clock, FileDiff, Users]);

/// [`gpui_cn::CnAssets`] plus the icons the rule builder story uses for
/// its fields and actions.
#[derive(Clone, Copy, Debug, Default)]
pub struct StoryAssets;

impl AssetSource for StoryAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = StoryExtras.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_cn::CnAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_cn::CnAssets.list(path)?;
        paths.extend(StoryExtras.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
