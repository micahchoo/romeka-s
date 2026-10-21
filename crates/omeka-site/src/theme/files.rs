//! Every file the bundled theme ships, with its bytes compiled in.
//!
//! The theme's own sheet and placeholder come from [`super::style`]. The IIIF
//! viewers are vendored under `crates/omeka-site/vendor/` by
//! `vendor/update.sh`, at the versions `NOTICE` records: Mirador for the viewer
//! page and OpenSeadragon, with its button images, for the IIIF image block and
//! the IIIF image media renderer; model-viewer for a 3D file's page, which the
//! page loads only when the reader asks to turn the model; and Leaflet, with
//! its stylesheet, for maps. They are libraries, so the document frame never
//! links them; the view that needs one loads it. The webfonts come last, from
//! [`super::fonts`].

use super::fonts::FONT_ASSETS;
use super::style::{PLACEHOLDER_SVG, STYLE_CSS};
use super::{AssetKind, ThemeAsset};

/// A vendored file, by its path under both `vendor/` and the asset base.
macro_rules! vendored {
    ($kind:ident, $path:expr) => {
        ThemeAsset {
            path: concat!("vendor/", $path),
            kind: AssetKind::$kind,
            content: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/", $path)),
        }
    };
}

/// One of OpenSeadragon's navigation images, which it loads from `prefixUrl`.
macro_rules! png {
    ($name:literal) => {
        vendored!(Png, concat!("openseadragon/images/", $name))
    };
}

/// The bundled theme's files, in the order the frame links its head assets.
pub(super) static FILES: [ThemeAsset; OWN.len() + FONT_ASSETS.len()] = {
    let mut out = [OWN[0]; OWN.len() + FONT_ASSETS.len()];
    let mut index = 0;
    while index < OWN.len() {
        out[index] = OWN[index];
        index += 1;
    }
    while index < out.len() {
        out[index] = FONT_ASSETS[index - OWN.len()];
        index += 1;
    }
    out
};

/// The theme's own sheet, placeholder and script, then the vendored files.
const OWN: [ThemeAsset; 54] = [
    ThemeAsset {
        path: "css/style.css",
        kind: AssetKind::Stylesheet,
        content: STYLE_CSS.as_bytes(),
    },
    ThemeAsset {
        path: "img/placeholder.svg",
        kind: AssetKind::Image,
        content: PLACEHOLDER_SVG.as_bytes(),
    },
    ThemeAsset {
        path: "js/iiif-viewer.js",
        kind: AssetKind::Library,
        content: crate::iiif_viewer::INIT_SCRIPT.as_bytes(),
    },
    ThemeAsset {
        path: "js/map.js",
        kind: AssetKind::Library,
        content: crate::map::MAP_SCRIPT.as_bytes(),
    },
    ThemeAsset {
        path: "js/lightbox.js",
        kind: AssetKind::Library,
        content: crate::blocks::gallery::LIGHTBOX_SCRIPT.as_bytes(),
    },
    ThemeAsset {
        path: "js/carousel.js",
        kind: AssetKind::Library,
        content: crate::blocks::carousel::SCRIPT.as_bytes(),
    },
    ThemeAsset {
        path: "js/model.js",
        kind: AssetKind::Library,
        content: crate::model_reader::SCRIPT.as_bytes(),
    },
    ThemeAsset {
        path: "js/annotations.js",
        kind: AssetKind::Library,
        content: crate::annotation::SCRIPT.as_bytes(),
    },
    ThemeAsset {
        path: "js/share.js",
        kind: AssetKind::Library,
        content: crate::share::SCRIPT.as_bytes(),
    },
    vendored!(Library, "leaflet/leaflet.js"),
    vendored!(LibraryStylesheet, "leaflet/leaflet.css"),
    vendored!(Library, "mirador/mirador.min.js"),
    vendored!(Library, "openseadragon/openseadragon.min.js"),
    vendored!(Library, "model-viewer/model-viewer-umd.min.js"),
    png!("button_grouphover.png"),
    png!("button_hover.png"),
    png!("button_pressed.png"),
    png!("button_rest.png"),
    png!("flip_grouphover.png"),
    png!("flip_hover.png"),
    png!("flip_pressed.png"),
    png!("flip_rest.png"),
    png!("fullpage_grouphover.png"),
    png!("fullpage_hover.png"),
    png!("fullpage_pressed.png"),
    png!("fullpage_rest.png"),
    png!("home_grouphover.png"),
    png!("home_hover.png"),
    png!("home_pressed.png"),
    png!("home_rest.png"),
    png!("next_grouphover.png"),
    png!("next_hover.png"),
    png!("next_pressed.png"),
    png!("next_rest.png"),
    png!("previous_grouphover.png"),
    png!("previous_hover.png"),
    png!("previous_pressed.png"),
    png!("previous_rest.png"),
    png!("rotateleft_grouphover.png"),
    png!("rotateleft_hover.png"),
    png!("rotateleft_pressed.png"),
    png!("rotateleft_rest.png"),
    png!("rotateright_grouphover.png"),
    png!("rotateright_hover.png"),
    png!("rotateright_pressed.png"),
    png!("rotateright_rest.png"),
    png!("zoomin_grouphover.png"),
    png!("zoomin_hover.png"),
    png!("zoomin_pressed.png"),
    png!("zoomin_rest.png"),
    png!("zoomout_grouphover.png"),
    png!("zoomout_hover.png"),
    png!("zoomout_pressed.png"),
    png!("zoomout_rest.png"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every vendored library, stylesheet and image is served, so a refresh
    /// that adds a file cannot leave it a 404. The licences are not served.
    #[test]
    fn every_vendored_file_is_listed() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor");
        let mut on_disk = Vec::new();
        let mut folders = vec![std::path::PathBuf::from(root)];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).expect("the vendor folder") {
                let path = entry.expect("an entry").path();
                if path.is_dir() {
                    folders.push(path);
                } else if matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("js" | "css" | "png")
                ) {
                    let relative = path.strip_prefix(root).expect("under the root");
                    on_disk.push(format!("vendor/{}", relative.display()));
                }
            }
        }
        on_disk.sort();
        let mut listed: Vec<String> = FILES
            .iter()
            .filter(|file| file.path.starts_with("vendor/"))
            .map(|file| file.path.to_string())
            .collect();
        listed.sort();
        assert_eq!(listed, on_disk);
    }
}
