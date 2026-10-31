//! Windows path comparison without Windows calls. Paths are compared without
//! regard to case, with either slash, and without trailing separators.

/// A path prepared for comparison: lower case, backslashes, no trailing
/// separator.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NormPath(String);

impl NormPath {
    /// Normalises a path.
    pub fn new(path: &str) -> Self {
        let mut s: String = path
            .trim()
            .chars()
            .map(|c| if c == '/' { '\\' } else { c })
            .flat_map(char::to_lowercase)
            .collect();
        while s.len() > 3 && s.ends_with('\\') {
            s.pop();
        }
        Self(s)
    }

    /// The normalised text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this path is `folder` or lies inside it.
    pub fn is_within(&self, folder: &Self) -> bool {
        if folder.0.is_empty() {
            return false;
        }
        self.0 == folder.0
            || self
                .0
                .strip_prefix(&folder.0)
                .is_some_and(|rest| rest.starts_with('\\') || folder.0.ends_with('\\'))
    }

    /// The containing folder, if any.
    pub fn parent(&self) -> Option<Self> {
        self.0
            .rfind('\\')
            .filter(|&i| i > 0)
            .map(|i| Self(self.0[..i].to_owned()))
    }

    /// The last component, for example `cyberpunk2077.exe`.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('\\').next().unwrap_or(&self.0)
    }
}

/// The last component of a path as written, keeping its case.
pub fn display_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises() {
        assert_eq!(
            NormPath::new("D:/Games/Foo/").as_str(),
            r"d:\games\foo",
            "slashes, case, trailing"
        );
        assert_eq!(NormPath::new(r"C:\").as_str(), r"c:\", "root kept");
    }

    #[test]
    fn within() {
        let folder = NormPath::new(r"D:\SteamLibrary\steamapps\common\Cyberpunk 2077");
        let exe = NormPath::new(
            r"d:\steamlibrary\steamapps\common\cyberpunk 2077\bin\x64\Cyberpunk2077.exe",
        );
        assert!(exe.is_within(&folder), "inside");
        let sibling = NormPath::new(r"D:\SteamLibrary\steamapps\common\Cyberpunk 2077 Tools\a.exe");
        assert!(
            !sibling.is_within(&folder),
            "prefix of a name is not inside"
        );
        assert!(
            NormPath::new(r"C:\x\a.exe").is_within(&NormPath::new(r"C:\")),
            "drive root"
        );
    }

    #[test]
    fn parts() {
        let exe = NormPath::new(r"D:\Games\Foo\Foo.exe");
        assert_eq!(exe.file_name(), "foo.exe", "file name");
        assert_eq!(
            exe.parent().map(|p| p.as_str().to_owned()),
            Some(r"d:\games\foo".to_owned()),
            "parent"
        );
        assert_eq!(
            display_name(r"D:\Games\Foo\Foo.exe"),
            "Foo.exe",
            "case kept"
        );
    }
}
