//! Turn the icon SVGs into the rasters Windows wants: the `.ico` the shell
//! reads and the PNGs an MSIX package declares.
//!
//! Linux ships the SVGs and lets the desktop rasterize them. Windows has no
//! such step: the shell reads a fixed set of sizes out of an icon directory and
//! the Store reads named PNGs at fixed dimensions, so every size has to exist
//! before the file is shipped. This is the step between, and it is why
//! rasterized artifacts are committed to a repository that otherwise holds only
//! sources.
//!
//! **Three drawings, not one, and that is the difference from
//! slipcase-desktop's copy of this tool.** That application claims one
//! extension; Segler claims two, so `packaging/linux/icons` holds an
//! application icon and a document icon per extension, and the package needs a
//! file-type logo for each. Everything else here is that tool unchanged.
//!
//! The name is narrower than the job and is kept anyway: `windows.yml` runs
//! this package by name and `packaging/windows/README.md` refers to it, and
//! renaming a tool to widen its remit costs more than the sentence it saves.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use resvg::tiny_skia;
use resvg::usvg;

/// The sizes the Windows shell asks for.
///
/// 16, 32, and 48 are the list, the desktop, and the tile; 256 is what the
/// extra-large view and the file properties dialog use. 20, 24, 40, 64, and
/// 128 are the same three sizes again at the display scalings Windows offers,
/// and without them the shell picks a neighbour and resamples it, which is
/// visibly softer than rendering the vector at the size wanted.
const SIZES: &[u32] = &[16, 20, 24, 32, 40, 48, 64, 128, 256];

/// Above this, an entry is stored as PNG rather than as a bitmap.
///
/// PNG entries are read by Vista and later and nothing older, and a 256-pixel
/// bitmap entry costs 256 KiB where the PNG costs a few. Below the line the
/// bitmap is kept, because it is the format every consumer of an icon
/// directory has always understood and the saving there is not worth anything.
const PNG_ABOVE: u32 = 48;

/// Which of the three drawings an asset is rendered from.
///
/// The application icon is the boat in a roundel and stands for Segler itself:
/// the tile, the store logo, the entry in the application list. The two
/// document drawings are a page carrying the same boat, blue for an archive and
/// cream for bare markup, and each is the logo of one `uap:FileTypeAssociation`
/// in `AppxManifest.xml.in`. Handing the archive's drawing to both associations
/// would be the same defect the Linux icons exist to avoid: two kinds that do
/// not tell apart in a listing.
#[derive(Clone, Copy)]
enum Drawing {
    Application,
    Archive,
    Document,
}

/// The three drawings, in the order the icon directories are written.
const DRAWINGS: &[Drawing] = &[Drawing::Application, Drawing::Archive, Drawing::Document];

impl Drawing {
    /// The file in `packaging/linux/icons`, which is the source for every
    /// platform's icons and is not duplicated here.
    fn file(self) -> &'static str {
        match self {
            Drawing::Application => "segler-desktop.svg",
            Drawing::Archive => "application-vnd.doclang.archive+zip.svg",
            Drawing::Document => "application-vnd.doclang.document+xml.svg",
        }
    }

    /// The icon directory written for it.
    ///
    /// Three where slipcase-desktop has one, and the reason is the side-loaded
    /// install rather than the package. A packaged file type takes its icon
    /// from `uap:Logo`, so the PNGs above would be enough for the Store - but
    /// `install.ps1` registers the same two types through `HKCU`, where an icon
    /// is a `path,index` into an icon directory, and pointing both ProgIDs at
    /// the application's drawing would put the same picture on a `.dclx` and a
    /// `.dclg` in Explorer. The manifest and the script are meant to claim the
    /// same things with the same faces; a person who has both installed should
    /// not be able to tell which one won by looking.
    fn ico(self) -> &'static str {
        match self {
            Drawing::Application => "segler.ico",
            Drawing::Archive => "dclx.ico",
            Drawing::Document => "dclg.ico",
        }
    }
}

/// One of the images `AppxManifest.xml.in` names, at the size the Store wants.
struct Asset {
    stem: &'static str,
    from: Drawing,
    width: u32,
    height: u32,
    /// How much of the shorter side the drawing occupies, centred.
    fill: f32,
    /// Whether the shell ever draws this one as a bare icon rather than on a
    /// tile. Only `Square44x44Logo` is, and only that one gets the target-size
    /// and unplated variants below.
    icon: bool,
}

/// The display scalings Windows offers, as the Store spells them.
///
/// Without these there is one bitmap per asset and every other scaling is an
/// upscale of it. `segler.ico` carries nine sizes for exactly this reason and
/// the argument does not change because the file is a PNG.
const SCALES: &[u32] = &[100, 125, 150, 200, 400];

/// The sizes the shell asks for when it wants an icon rather than a tile.
const TARGET_SIZES: &[u32] = &[16, 24, 32, 48, 256];

/// The three forms of each target size, and the reason this list exists.
///
/// `BackgroundColor` in `AppxManifest.xml` is `transparent`, so where Windows
/// draws a *plated* icon it fills the plate with the user's accent colour. On
/// slipcase-desktop's taskbar that put the drawing on a purple square -
/// measured there on 2026-08-28 against an accent of `#744DA9` - while the
/// side-loaded install drew the same icon unplated from the `.ico`. One
/// application with two faces, which is what the manifest's own comment about
/// matching `install.ps1` exists to prevent.
///
/// An `altform-unplated` asset is what tells the shell not to plate. The light
/// variant is the same pixels: this drawing is coloured rather than monochrome,
/// so it needs no separate treatment for a light taskbar - but the qualifier
/// has to exist or Windows 11 falls back to the plated form there.
const ALTFORMS: &[&str] = &["", "_altform-unplated", "_altform-lightunplated"];

/// The six images the manifest names, and nothing else.
///
/// The dimensions are the Store's and are not a choice. `fill` is: a tile is
/// drawn on a coloured plate and Microsoft's tile guidance leaves the icon
/// about two thirds of it, where an icon-shaped asset - the store logo, the
/// application list entry, a file type - is drawn at the size it is given and
/// wants the whole of it, which is also what `segler.ico` does at every size it
/// holds. Only a look at real tiles settles the two thirds, and that look was
/// taken on slipcase-desktop and settled them there.
///
/// `dclx` and `dclg` are the two file-type logos, one per association. They are
/// 256 because that is the largest the shell asks for and the only place a
/// document icon is ever drawn big; the `scale-` variants beside them come from
/// the same loop as everything else and are resolved through `resources.pri`,
/// which `build-msix.ps1` builds.
const ASSETS: &[Asset] = &[
    Asset { stem: "StoreLogo", from: Drawing::Application, width: 50, height: 50, fill: 1.00, icon: false },
    Asset { stem: "Square44x44Logo", from: Drawing::Application, width: 44, height: 44, fill: 1.00, icon: true },
    Asset { stem: "Square150x150Logo", from: Drawing::Application, width: 150, height: 150, fill: 0.66, icon: false },
    Asset { stem: "Wide310x150Logo", from: Drawing::Application, width: 310, height: 150, fill: 0.66, icon: false },
    Asset { stem: "dclx", from: Drawing::Archive, width: 256, height: 256, fill: 1.00, icon: false },
    Asset { stem: "dclg", from: Drawing::Document, width: 256, height: 256, fill: 1.00, icon: false },
];

/// The sizes Partner Center's *Store logo* listing field accepts.
///
/// Both, rather than only the smaller: the field takes either and the larger is
/// what survives a future listing that wants more pixels. Neither goes in the
/// package - see the comment where they are written.
const LISTING_SIZES: &[u32] = &[1080, 2160];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut args = std::env::args_os().skip(1);
    // The directory rather than one file, because three drawings are involved.
    let icons = args
        .next()
        .map_or_else(|| here.join("../../linux/icons"), PathBuf::from);
    // The directory the three `.ico` files are written into, rather than one
    // file's path, for the same reason the input is a directory.
    let destination = args.next().map_or_else(|| here.join(".."), PathBuf::from);
    let assets = args
        .next()
        .map_or_else(|| here.join("../assets"), PathBuf::from);
    let listing = args
        .next()
        .map_or_else(|| here.join("../listing"), PathBuf::from);

    let read = |drawing: Drawing| -> Result<usvg::Tree, Box<dyn std::error::Error>> {
        let path = icons.join(drawing.file());
        let svg = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(usvg::Tree::from_data(&svg, &usvg::Options::default())?)
    };
    let application = read(Drawing::Application)?;
    let archive = read(Drawing::Archive)?;
    let document = read(Drawing::Document)?;
    let tree = |drawing: Drawing| match drawing {
        Drawing::Application => &application,
        Drawing::Archive => &archive,
        Drawing::Document => &document,
    };

    // One icon directory per drawing: the application's, which is also the
    // window's icon and the Start menu shortcut's, and one per file type for
    // `install.ps1` to point a ProgID at. `Drawing::ico` argues why there are
    // three rather than one.
    std::fs::create_dir_all(&destination)?;
    for &drawing in DRAWINGS {
        let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
        for &size in SIZES {
            let image = icon_image(tree(drawing), size)?;
            let entry = if size > PNG_ABOVE {
                ico::IconDirEntry::encode_as_png(&image)?
            } else {
                ico::IconDirEntry::encode_as_bmp(&image)?
            };
            icon.add_entry(entry);
        }
        let path = destination.join(drawing.ico());
        icon.write(std::fs::File::create(&path)?)?;
        println!(
            "wrote {} - {} sizes: {}",
            path.display(),
            SIZES.len(),
            SIZES
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Written fresh every time. A renamed or dropped asset would otherwise be
    // left behind by a rebuild, and `windows.yml` compares this directory
    // against a rebuild - so a stale file would sit in the package and in the
    // tree with nothing objecting to it.
    if assets.exists() {
        std::fs::remove_dir_all(&assets)?;
    }
    std::fs::create_dir_all(&assets)?;

    let mut written = 0usize;
    for asset in ASSETS {
        let drawing = tree(asset.from);
        // The unqualified name as well as the qualified ones. The manifest
        // names this one, and it is what resolves when nothing indexes the
        // package - so keeping it means the assets are correct with or without
        // `resources.pri`, rather than only with.
        write_png(
            drawing,
            &assets,
            &format!("{}.png", asset.stem),
            asset.width,
            asset.height,
            asset.fill,
        )?;
        written += 1;

        for &scale in SCALES {
            let (w, h) = (scaled(asset.width, scale), scaled(asset.height, scale));
            let name = format!("{}.scale-{}.png", asset.stem, scale);
            write_png(drawing, &assets, &name, w, h, asset.fill)?;
            written += 1;
        }

        if asset.icon {
            for &size in TARGET_SIZES {
                for altform in ALTFORMS {
                    let name = format!("{}.targetsize-{}{}.png", asset.stem, size, altform);
                    write_png(drawing, &assets, &name, size, size, 1.0)?;
                    written += 1;
                }
            }
        }
    }
    println!("wrote {} - {} images", assets.display(), written);

    // The store listing logo, which is not in the package and must not be.
    //
    // Partner Center's *Store logo* field is a listing image rather than a
    // package asset: it refuses anything but 1080x1080 or 2160x2160, measured
    // on slipcase-desktop on 2026-08-29 against the live form after that
    // repository had assumed the 300x300 the older documentation describes.
    // Writing it beside the package assets rather than into them is deliberate
    // - a file added to `assets` lands in the MSIX, and a package that gains a
    // file has to be certified again for an image no installed copy would ever
    // read.
    if listing.exists() {
        std::fs::remove_dir_all(&listing)?;
    }
    std::fs::create_dir_all(&listing)?;
    for &size in LISTING_SIZES {
        write_png(
            &application,
            &listing,
            &format!("store-logo-{size}.png"),
            size,
            size,
            1.0,
        )?;
    }
    println!("wrote {} - {} images", listing.display(), LISTING_SIZES.len());

    Ok(())
}

/// One asset dimension at one scaling, the way the Store rounds it.
///
/// 50 at 125% is 62.5 and the Store's own table says 63, so this rounds rather
/// than truncating. Getting it wrong by a pixel is not a build failure - it is
/// an image the shell quietly rescales, which is the whole thing these variants
/// exist to avoid.
fn scaled(size: u32, scale: u32) -> u32 {
    (f64::from(size) * f64::from(scale) / 100.0).round() as u32
}

fn write_png(
    tree: &usvg::Tree,
    dir: &Path,
    name: &str,
    width: u32,
    height: u32,
    fill: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let pixmap = draw(tree, width, height, fill)?;
    std::fs::write(dir.join(name), pixmap.encode_png()?)?;
    Ok(())
}

/// The drawing centred on a canvas of the given size.
///
/// Every raster here comes through this, so the wide tile is the same rendering
/// as the square one rather than a square image somebody stretched. The
/// drawings are square and one of the canvases is not, which is the whole
/// reason this takes a width and a height rather than a size.
fn draw(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
    fill: f32,
) -> Result<tiny_skia::Pixmap, Box<dyn std::error::Error>> {
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or("a pixmap of that size could not be made")?;

    // The SVGs are drawn on a 64-unit grid; this is the only scaling involved.
    #[allow(clippy::cast_precision_loss)]
    let (w, h) = (width as f32, height as f32);
    let side = w.min(h) * fill;
    let scale = side / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_translate((w - side) / 2.0, (h - side) / 2.0)
            .pre_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

/// The drawing at one size, as straight rather than premultiplied alpha.
///
/// `tiny_skia` renders into premultiplied pixels and an icon directory holds
/// straight ones. Handing the pixmap's bytes over unconverted looks right
/// everywhere the drawing is opaque and wrong along every antialiased edge,
/// which on this icon is its entire outline. Its own PNG encoder does the same
/// conversion, which is why the assets above need no equivalent.
fn icon_image(
    tree: &usvg::Tree,
    size: u32,
) -> Result<ico::IconImage, Box<dyn std::error::Error>> {
    let pixmap = draw(tree, size, size, 1.0)?;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for pixel in pixmap.pixels() {
        let straight = pixel.demultiply();
        rgba.extend_from_slice(&[
            straight.red(),
            straight.green(),
            straight.blue(),
            straight.alpha(),
        ]);
    }
    Ok(ico::IconImage::from_rgba_data(size, size, rgba))
}
