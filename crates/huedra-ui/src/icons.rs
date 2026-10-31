//! Finds an icon for each game on a background thread: one kept from an
//! earlier run, else Discord's list (when online lookups are on), else the
//! icon inside the game's program. Icons are kept as PNG files for 30 days.
//! Feature doc: `docs/features/13-game-info.md`.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use huedra_core::icons::{self, Catalog, IconEntry, IconIndex, IconSource};
use huedra_core::library::{Game, GameId, Store, xbox};
use huedra_win::{http, shell, stores};

use crate::lookups::now_secs;

/// Time between two icon downloads.
const PACE: Duration = Duration::from_millis(200);
/// Decoded images may use at most this much memory.
const MAX_DECODE_BYTES: usize = 4 << 20;
/// Downloaded icons larger than this are refused.
const MAX_SIDE: u32 = 512;

/// An icon ready to show.
pub struct Icon {
    pub id: GameId,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Starts the worker. `deliver` receives each icon on the worker thread and
/// must hand it to the window itself.
pub fn spawn(
    games: Vec<Game>,
    folder: PathBuf,
    online: bool,
    deliver: impl Fn(Icon) + Send + 'static,
) {
    let _ = thread::Builder::new()
        .name("huedra-icons".into())
        .spawn(move || run(&games, &folder, online, &deliver));
}

fn run(games: &[Game], folder: &Path, online: bool, deliver: &dyn Fn(Icon)) {
    let index_path = folder.join("index.toml");
    let mut index = IconIndex::load(&index_path);
    let now = now_secs();

    // Icons kept from earlier runs first, so the list fills at once.
    let mut todo = Vec::new();
    for game in games {
        let file = folder.join(icons::file_name(&game.id));
        match index.get(&game.id) {
            Some(entry) if entry.is_current(now, online) => {
                if entry.source != IconSource::None
                    && let Some(icon) = load(&game.id, &file)
                {
                    deliver(icon);
                }
            }
            _ => todo.push(game),
        }
    }
    if todo.is_empty() {
        return;
    }

    // Discord's list is fetched once, only when some game needs it.
    let catalog = if online { fetch_catalog() } else { None };
    let steam = stores::steam_root();
    for game in todo {
        let file = folder.join(icons::file_name(&game.id));
        let app = catalog.as_ref().and_then(|c| c.find(game));
        let mut source = IconSource::None;
        if let Some(path) = app.and_then(icons::App::icon_path)
            && let Some(png) = download(&path)
            && decode(&png).is_some()
            && write(&file, &png)
        {
            source = IconSource::Discord;
        }
        if source == IconSource::None
            && let Some(app_id) = game.id.steam_app_id()
            && let Some(root) = &steam
            && let Some(jpeg) = icons::steam_cached_icon(root, app_id)
            && let Some(png) = jpeg_as_png(&jpeg)
            && write(&file, &png)
        {
            source = IconSource::Steam;
        }
        if source == IconSource::None
            && game.store == Store::Xbox
            && let Some(png) = package_logo(Path::new(&game.install_dir))
            && write(&file, &png)
        {
            source = IconSource::Program;
        }
        if source == IconSource::None {
            let hints = app.map(|a| a.programs().to_vec()).unwrap_or_default();
            let program = game.exe.as_ref().map(PathBuf::from).or_else(|| {
                (!game.install_dir.is_empty())
                    .then(|| icons::find_program(Path::new(&game.install_dir), &hints, &game.name))
                    .flatten()
            });
            if let Some(program) = program
                && let Some(rgba) = shell::program_icon(&program.to_string_lossy(), icons::SIZE)
                && let Some(png) = encode(&rgba, icons::SIZE)
                && write(&file, &png)
            {
                source = IconSource::Program;
            }
        }
        if source == IconSource::None {
            let _ = fs::remove_file(&file);
        } else if let Some(icon) = load(&game.id, &file) {
            deliver(icon);
        }
        index.put(IconEntry {
            id: game.id.clone(),
            source,
            online: catalog.is_some(),
            checked: now,
        });
        let _ = index.save(&index_path);
    }
}

/// The logo an Xbox game names in its `MicrosoftGame.config`, as PNG.
fn package_logo(content: &Path) -> Option<Vec<u8>> {
    let config = huedra_core::files::read_limited(&content.join("MicrosoftGame.config"), 1 << 20)
        .ok()
        .flatten()?;
    xbox::logo_files(&config).into_iter().find_map(|name| {
        let path = content.join(name);
        if fs::metadata(&path).ok()?.len() > icons::MAX_ICON_BYTES as u64 {
            return None;
        }
        let bytes = fs::read(path).ok()?;
        decode(&bytes).is_some().then_some(bytes)
    })
}

fn user_agent() -> String {
    format!(
        "Huedra/{} (Windows HDR switching; game icons)",
        huedra_core::VERSION
    )
}

fn fetch_catalog() -> Option<Catalog> {
    let response = http::get(
        icons::LIST_HOST,
        icons::LIST_PATH,
        &user_agent(),
        icons::MAX_LIST_BYTES,
    )
    .ok()?;
    (response.status == 200)
        .then(|| Catalog::parse(&response.body))
        .flatten()
}

fn download(path: &str) -> Option<Vec<u8>> {
    thread::sleep(PACE);
    let response = http::get(icons::ICON_HOST, path, &user_agent(), icons::MAX_ICON_BYTES).ok()?;
    (response.status == 200).then_some(response.body)
}

fn write(file: &Path, png: &[u8]) -> bool {
    huedra_core::files::write_atomic(file, png).is_ok()
}

fn load(id: &GameId, file: &Path) -> Option<Icon> {
    let bytes = fs::read(file).ok()?;
    let (rgba, width, height) = decode(&bytes)?;
    Some(Icon {
        id: id.clone(),
        rgba,
        width,
        height,
    })
}

/// Decodes a PNG into straight RGBA, refusing large or malformed images.
fn decode(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: MAX_DECODE_BYTES,
        },
    );
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = reader.info().size();
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    let mut buffer = vec![0_u8; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let pixels = &buffer[..frame.buffer_size()];
    let rgba = match frame.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((rgba, frame.width, frame.height))
}

/// Steam's cached JPEG icon, converted to PNG.
fn jpeg_as_png(path: &Path) -> Option<Vec<u8>> {
    if fs::metadata(path).ok()?.len() > icons::MAX_ICON_BYTES as u64 {
        return None;
    }
    let bytes = fs::read(path).ok()?;
    let options = zune_jpeg::zune_core::options::DecoderOptions::default()
        .jpeg_set_out_colorspace(zune_jpeg::zune_core::colorspace::ColorSpace::RGBA)
        .set_max_width(MAX_SIDE as usize)
        .set_max_height(MAX_SIDE as usize);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(Cursor::new(bytes), options);
    let rgba = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    (width == height && rgba.len() == (width * height * 4) as usize)
        .then(|| encode(&rgba, width))
        .flatten()
}

/// Encodes square RGBA pixels as a PNG.
fn encode(rgba: &[u8], side: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, side, side);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(rgba).ok()?;
    writer.finish().ok()?;
    Some(out)
}
