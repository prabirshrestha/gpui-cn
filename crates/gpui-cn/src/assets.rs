use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString, assets::Assets};

gpui_kit::assets::icon_assets!(
    CnExtras,
    [
        ClipboardList,
        FileCode,
        FileSpreadsheet,
        FolderPlus,
        Gauge,
        GitBranch,
        GripVertical,
        Hand,
        Image,
        Laptop,
        Lock,
        LockOpen,
        Mic,
        Presentation,
        ShieldAlert,
        ShieldCheck,
        Video,
    ]
);

/// The asset source of an application that uses gpui-cn: the default
/// `gpui_kit::assets::Assets` plus the Lucide icons gpui-cn's components
/// draw by default that the default set does not hold, such as the
/// composer's microphone, shield, file kinds, and git branch, and the rule
/// builder's grip and new-group icon.
///
/// An application that does not register it still works: it passes its own
/// icons to the components that take one, and a default icon that is not
/// registered renders nothing.
///
/// ```no_run
/// let app = gpui_kit::application().with_assets(gpui_cn::CnAssets);
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct CnAssets;

impl AssetSource for CnAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = CnExtras.load(path)? {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Assets.list(path)?;
        paths.extend(CnExtras.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
