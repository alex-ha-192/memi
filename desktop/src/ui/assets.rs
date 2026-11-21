//! Icons (Lucide, ISC), the elephant (Microsoft Fluent Emoji, MIT) and fonts, built into the app.

use gpui::{AssetSource, SharedString};
use std::borrow::Cow;

const FILES: &[(&str, &[u8])] = &[
    ("icons/folder.svg", include_bytes!("../../assets/icons/folder.svg")),
    ("icons/server.svg", include_bytes!("../../assets/icons/server.svg")),
    ("icons/arrow-down.svg", include_bytes!("../../assets/icons/arrow-down.svg")),
    ("icons/arrow-up.svg", include_bytes!("../../assets/icons/arrow-up.svg")),
    ("icons/calendar.svg", include_bytes!("../../assets/icons/calendar.svg")),
    ("icons/camera.svg", include_bytes!("../../assets/icons/camera.svg")),
    ("icons/check.svg", include_bytes!("../../assets/icons/check.svg")),
    ("icons/chevron-left.svg", include_bytes!("../../assets/icons/chevron-left.svg")),
    ("icons/chevron-right.svg", include_bytes!("../../assets/icons/chevron-right.svg")),
    ("icons/chevron-down.svg", include_bytes!("../../assets/icons/chevron-down.svg")),
    ("icons/circle-check.svg", include_bytes!("../../assets/icons/circle-check.svg")),
    ("icons/circle-x.svg", include_bytes!("../../assets/icons/circle-x.svg")),
    ("icons/cloud.svg", include_bytes!("../../assets/icons/cloud.svg")),
    ("icons/settings.svg", include_bytes!("../../assets/icons/settings.svg")),
    ("icons/cloud-off.svg", include_bytes!("../../assets/icons/cloud-off.svg")),
    ("icons/cloud-check.svg", include_bytes!("../../assets/icons/cloud-check.svg")),
    ("icons/cloud-alert.svg", include_bytes!("../../assets/icons/cloud-alert.svg")),
    ("icons/copy.svg", include_bytes!("../../assets/icons/copy.svg")),
    ("icons/ellipsis.svg", include_bytes!("../../assets/icons/ellipsis.svg")),
    ("icons/image-plus.svg", include_bytes!("../../assets/icons/image-plus.svg")),
    ("icons/key-round.svg", include_bytes!("../../assets/icons/key-round.svg")),
    ("icons/monitor.svg", include_bytes!("../../assets/icons/monitor.svg")),
    ("icons/monitor-smartphone.svg", include_bytes!("../../assets/icons/monitor-smartphone.svg")),
    ("icons/moon.svg", include_bytes!("../../assets/icons/moon.svg")),
    ("icons/palette.svg", include_bytes!("../../assets/icons/palette.svg")),
    ("icons/pencil.svg", include_bytes!("../../assets/icons/pencil.svg")),
    ("icons/pin.svg", include_bytes!("../../assets/icons/pin.svg")),
    ("icons/plus.svg", include_bytes!("../../assets/icons/plus.svg")),
    ("icons/qr-code.svg", include_bytes!("../../assets/icons/qr-code.svg")),
    ("icons/refresh-cw.svg", include_bytes!("../../assets/icons/refresh-cw.svg")),
    ("icons/search.svg", include_bytes!("../../assets/icons/search.svg")),
    ("icons/sparkles.svg", include_bytes!("../../assets/icons/sparkles.svg")),
    ("icons/sun.svg", include_bytes!("../../assets/icons/sun.svg")),
    ("icons/trash-2.svg", include_bytes!("../../assets/icons/trash-2.svg")),
    ("icons/user.svg", include_bytes!("../../assets/icons/user.svg")),
    ("icons/x.svg", include_bytes!("../../assets/icons/x.svg")),
    ("mascot.png", include_bytes!("../../assets/mascot.png")),
    ("app-icon.png", include_bytes!("../../assets/app-icon.png")),
];

pub const FONTS: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/plus_jakarta_sans_regular.ttf"),
    include_bytes!("../../assets/fonts/plus_jakarta_sans_medium.ttf"),
    include_bytes!("../../assets/fonts/plus_jakarta_sans_semibold.ttf"),
    include_bytes!("../../assets/fonts/plus_jakarta_sans_bold.ttf"),
    include_bytes!("../../assets/fonts/plus_jakarta_sans_extrabold.ttf"),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES.iter().find(|(p, _)| *p == path).map(|(_, b)| Cow::Borrowed(*b)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(FILES.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| SharedString::from(*p)).collect())
    }
}
