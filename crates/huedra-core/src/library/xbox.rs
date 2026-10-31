//! Xbox app (Microsoft Store PC games): each drive's `.GamingRoot` names the
//! game folders; each game has `Content\MicrosoftGame.config`.

use super::{Game, GameId, Store};

/// Folders (relative to the drive root) listed in a `.GamingRoot` file:
/// `RGBX`, a 32-bit count, then null-terminated UTF-16 strings.
pub fn gaming_root_folders(bytes: &[u8]) -> Vec<String> {
    let Some(rest) = bytes.strip_prefix(b"RGBX") else {
        return Vec::new();
    };
    let Some((count, rest)) = rest.split_first_chunk::<4>() else {
        return Vec::new();
    };
    let count = u32::from_le_bytes(*count) as usize;
    let units: Vec<u16> = rest
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    units
        .split(|&u| u == 0)
        .filter(|s| !s.is_empty())
        .take(count.min(64))
        .map(String::from_utf16_lossy)
        .collect()
}

/// A game in `folder` (the folder holding `Content`). `config` is the text of
/// `Content\MicrosoftGame.config`; without it the folder is not a game.
pub fn game(folder_name: &str, folder: &str, config: &str) -> Option<Game> {
    let content = format!(r"{}\Content", folder.trim_end_matches(['\\', '/']));
    let name = attribute(config, "DefaultDisplayName").unwrap_or_else(|| folder_name.to_owned());
    let exe =
        element_attribute(config, "<Executable ", "Name").map(|exe| format!(r"{content}\{exe}"));
    Some(Game {
        id: GameId::new(Store::Xbox, folder_name),
        name,
        store: Store::Xbox,
        install_dir: content,
        exe,
    })
}

/// Image files a game's `MicrosoftGame.config` names as its logos, relative
/// to its `Content` folder, best for a small square icon first: the 44 px
/// logo at double scale, the 44 px logo, then the larger tiles. Names that
/// would leave the folder are left out.
pub fn logo_files(config: &str) -> Vec<String> {
    let mut files = Vec::new();
    for name in ["Square44x44Logo", "Square150x150Logo", "StoreLogo"] {
        let Some(file) = element_attribute(config, "<ShellVisuals", name) else {
            continue;
        };
        if file.contains("..") || file.contains(':') || file.starts_with(['\\', '/']) {
            continue;
        }
        if name == "Square44x44Logo"
            && let Some((stem, extension)) = file.rsplit_once('.')
        {
            files.push(format!("{stem}.scale-200.{extension}"));
        }
        files.push(file);
    }
    files
}

/// The value of the first `name="..."` attribute in the text.
fn attribute(xml: &str, name: &str) -> Option<String> {
    let start = xml.find(&format!("{name}=\""))? + name.len() + 2;
    let end = xml[start..].find('"')? + start;
    let value = unescape(&xml[start..end]);
    (!value.trim().is_empty()).then_some(value)
}

/// The value of `name` on the first element starting with `tag`.
fn element_attribute(xml: &str, tag: &str, name: &str) -> Option<String> {
    let at = xml.find(tag)?;
    let end = xml[at..].find('>')? + at;
    attribute(&xml[at..end], name)
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_files_in_order_and_inside_the_folder() {
        let config = r#"<Game><ShellVisuals DefaultDisplayName="Minecraft"
            Square150x150Logo="Logo.png" Square44x44Logo="Images\Small.png"
            StoreLogo="..\..\Windows\x.png"/></Game>"#;
        assert_eq!(
            logo_files(config),
            [
                r"Images\Small.scale-200.png",
                r"Images\Small.png",
                "Logo.png"
            ],
            "small first, a path leaving the folder left out"
        );
        assert!(logo_files("<Game/>").is_empty(), "no visuals");
    }

    #[test]
    fn reads_gaming_root() {
        let mut bytes = b"RGBX\x01\x00\x00\x00".to_vec();
        for unit in "XboxGames".encode_utf16().chain([0]) {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(
            gaming_root_folders(&bytes),
            vec!["XboxGames".to_owned()],
            "one folder"
        );
        assert!(gaming_root_folders(b"nope").is_empty(), "wrong header");
    }

    #[test]
    fn reads_config() {
        let config = r#"<Game><ExecutableList><Executable Name="Minecraft.Windows.exe" Id="Game"/></ExecutableList>
            <ShellVisuals DefaultDisplayName="Forza Horizon 5 &amp; More" /></Game>"#;
        let game = game("Forza Horizon 5", r"C:\XboxGames\Forza Horizon 5", config).expect("game");
        assert_eq!(game.name, "Forza Horizon 5 & More", "display name");
        assert_eq!(
            game.install_dir, r"C:\XboxGames\Forza Horizon 5\Content",
            "content folder"
        );
        assert_eq!(
            game.exe.as_deref(),
            Some(r"C:\XboxGames\Forza Horizon 5\Content\Minecraft.Windows.exe"),
            "executable"
        );
    }
}
