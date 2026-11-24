//! Emulators, listed like games so they get HDR rules of their own: found
//! through their uninstall entries, Scoop, and the folders their installers
//! use. `huedra-win` looks in those places; this module knows the names.
//! Feature doc: `docs/features/04-game-library.md`.

use super::registry::RegistryEntry;
use super::{Game, GameId, Store, clean_folder};
use crate::paths::NormPath;

/// One emulator Huedra knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Emulator {
    /// Its key in game ids (`emulator:<key>`), and its Scoop app name.
    pub key: &'static str,
    /// The name shown.
    pub name: &'static str,
    /// Its programs, the usual one first.
    pub programs: &'static [&'static str],
    /// Folders its installer uses: under Program Files, or a full path from
    /// the system drive (`\RetroArch-Win64`).
    pub folders: &'static [&'static str],
}

/// The emulators Huedra looks for.
pub const KNOWN: &[Emulator] = &[
    Emulator {
        key: "retroarch",
        name: "RetroArch",
        programs: &["retroarch.exe"],
        folders: &[r"\RetroArch-Win64", "RetroArch"],
    },
    Emulator {
        key: "dolphin",
        name: "Dolphin",
        programs: &["Dolphin.exe"],
        folders: &["Dolphin", "Dolphin-x64"],
    },
    Emulator {
        key: "pcsx2",
        name: "PCSX2",
        programs: &["pcsx2-qt.exe", "pcsx2.exe"],
        folders: &["PCSX2"],
    },
    Emulator {
        key: "rpcs3",
        name: "RPCS3",
        programs: &["rpcs3.exe"],
        folders: &["RPCS3"],
    },
    Emulator {
        key: "ppsspp",
        name: "PPSSPP",
        programs: &["PPSSPPWindows64.exe", "PPSSPPWindows.exe"],
        folders: &["PPSSPP"],
    },
    Emulator {
        key: "duckstation",
        name: "DuckStation",
        programs: &[
            "duckstation-qt-x64-ReleaseLTCG.exe",
            "duckstation-qt-x64-ReleaseLTCG-SSE2.exe",
        ],
        folders: &["DuckStation"],
    },
    Emulator {
        key: "cemu",
        name: "Cemu",
        programs: &["Cemu.exe"],
        folders: &["Cemu"],
    },
    Emulator {
        key: "xenia-canary",
        name: "Xenia Canary",
        programs: &["xenia_canary.exe"],
        folders: &[],
    },
    Emulator {
        key: "ryujinx",
        name: "Ryujinx",
        programs: &["Ryujinx.exe"],
        folders: &["Ryujinx"],
    },
    Emulator {
        key: "shadps4",
        name: "shadPS4",
        programs: &["shadPS4.exe", "shadps4.exe"],
        folders: &[],
    },
    Emulator {
        key: "xemu",
        name: "xemu",
        programs: &["xemu.exe"],
        folders: &["xemu"],
    },
];

/// The emulator an uninstall entry belongs to, with its folder: from
/// `InstallLocation`, or else the folder of the uninstaller.
pub fn from_uninstall(entry: &RegistryEntry) -> Option<(&'static Emulator, String)> {
    let shown = entry.get("DisplayName")?.to_lowercase();
    let emulator = KNOWN.iter().find(|e| {
        let name = e.name.to_lowercase();
        // "PCSX2 2.2.0", "RetroArch 1.19", but not "RetroArch Cores Pack".
        shown == name
            || shown
                .strip_prefix(&name)
                .is_some_and(|rest| rest.trim_start().starts_with(|c: char| c.is_ascii_digit()))
    })?;
    let folder = entry
        .get("InstallLocation")
        .map(clean_folder)
        .or_else(|| {
            let uninstaller = entry.get("UninstallString")?;
            let path = match uninstaller.strip_prefix('"') {
                Some(rest) => rest.split('"').next()?,
                None => uninstaller.split(" /").next()?,
            };
            let path = clean_folder(path);
            let cut = path.rfind('\\')?;
            Some(path[..cut].to_owned())
        })
        .filter(|f| !f.is_empty())?;
    Some((emulator, folder))
}

/// The emulator as a game: any program in `folder` counts as it.
pub fn game(emulator: &Emulator, folder: &str, exe: Option<String>) -> Game {
    Game {
        id: GameId::new(Store::Emulator, emulator.key),
        name: emulator.name.to_owned(),
        store: Store::Emulator,
        install_dir: clean_folder(folder),
        exe,
    }
}

/// One entry per emulator, leaving out those another store already lists
/// in the same folder (RetroArch from Steam).
pub fn dedupe(found: Vec<Game>, others: &[Game]) -> Vec<Game> {
    let taken = |dir: &str| {
        let dir = NormPath::new(dir);
        others
            .iter()
            .any(|g| g.store != Store::Emulator && NormPath::new(&g.install_dir) == dir)
    };
    let mut kept: Vec<Game> = Vec::new();
    for game in found {
        if !kept.iter().any(|k| k.id == game.id) && !taken(&game.install_dir) {
            kept.push(game);
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(values: &[(&str, &str)]) -> RegistryEntry {
        RegistryEntry {
            key: "k".into(),
            values: values
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn uninstall_entries_name_the_emulator_and_folder() {
        let (e, folder) = from_uninstall(&entry(&[
            ("DisplayName", "PCSX2 2.2.0"),
            ("InstallLocation", r"C:\Program Files\PCSX2\"),
        ]))
        .expect("PCSX2");
        assert_eq!(
            (e.key, folder.as_str()),
            ("pcsx2", r"C:\Program Files\PCSX2"),
            "install location"
        );

        let (e, folder) = from_uninstall(&entry(&[
            ("DisplayName", "RetroArch"),
            ("UninstallString", r#""C:\RetroArch-Win64\uninstall.exe""#),
        ]))
        .expect("RetroArch");
        assert_eq!(
            (e.key, folder.as_str()),
            ("retroarch", r"C:\RetroArch-Win64"),
            "uninstaller's folder"
        );

        let (_, folder) = from_uninstall(&entry(&[
            ("DisplayName", "PPSSPP"),
            ("UninstallString", r"C:\Games\PPSSPP\unins000.exe /SILENT"),
        ]))
        .expect("PPSSPP");
        assert_eq!(folder, r"C:\Games\PPSSPP", "unquoted uninstaller");
    }

    #[test]
    fn other_programs_are_not_emulators() {
        assert!(
            from_uninstall(&entry(&[
                ("DisplayName", "RetroArch Cores Pack"),
                ("InstallLocation", r"C:\x"),
            ]))
            .is_none(),
            "not RetroArch itself"
        );
        assert!(
            from_uninstall(&entry(&[
                ("DisplayName", "Dolphin Browser"),
                ("InstallLocation", r"C:\x"),
            ]))
            .is_none(),
            "another program"
        );
        assert!(
            from_uninstall(&entry(&[("DisplayName", "Cemu")])).is_none(),
            "no folder"
        );
    }

    #[test]
    fn games_have_stable_ids() {
        let g = game(&KNOWN[0], r"C:\RetroArch-Win64\", None);
        assert_eq!(g.id.as_str(), "emulator:retroarch", "id");
        assert_eq!(g.install_dir, r"C:\RetroArch-Win64", "folder");
        assert_eq!(g.store, Store::Emulator, "store");
    }

    #[test]
    fn dedupe_keeps_one_and_skips_store_copies() {
        let steam = Game {
            id: GameId::new(Store::Steam, "1118310"),
            name: "RetroArch".into(),
            store: Store::Steam,
            install_dir: r"D:\Steam\steamapps\common\RetroArch".into(),
            exe: None,
        };
        let found = vec![
            game(&KNOWN[0], r"D:\Steam\steamapps\common\RetroArch", None),
            game(&KNOWN[2], r"C:\PCSX2", None),
            game(&KNOWN[2], r"D:\Other\PCSX2", None),
        ];
        let kept = dedupe(found, &[steam]);
        assert_eq!(
            kept.len(),
            1,
            "Steam's RetroArch and the second PCSX2 left out"
        );
        assert_eq!(kept[0].install_dir, r"C:\PCSX2", "the first PCSX2");
    }

    #[test]
    fn keys_are_unique_and_lowercase() {
        for (i, e) in KNOWN.iter().enumerate() {
            assert_eq!(e.key, e.key.to_lowercase(), "lowercase key");
            assert!(!e.programs.is_empty(), "a program");
            assert!(KNOWN[i + 1..].iter().all(|o| o.key != e.key), "unique key");
        }
    }
}
