# Package manifests

Manifests for installing Huedra with a Windows package manager. They are not
published from here: each needs a submission by the owner.

## winget

`winget/manifests/a/apixelsnet/Huedra/1.2.1/` is a copy of what a pull request to
[microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) adds, at the
same path. The installer is the release zip with `huedra.exe` inside, installed
as a portable program with the command `huedra`.

For a new release: copy the folder to the new version, change `PackageVersion`,
`InstallerUrl`, `InstallerSha256` (`Get-FileHash` of the zip) and
`ReleaseNotesUrl`, then check it with `winget validate --manifest <folder>` and
`winget install --manifest <folder>` before opening the pull request.
`wingetcreate update apixelsnet.Huedra --version <version> --urls <zip url>`
does the same once the first version is in winget-pkgs.

## Scoop

`scoop/huedra.json` installs the release zip with a Start menu shortcut. With
`checkver` and `autoupdate`, a bucket picks up new releases by itself. It can be
added to a bucket of the project's own, or proposed to the Extras bucket.
Try it with `scoop install .\extras\packaging\scoop\huedra.json`.

## Updates

Huedra updates itself (Settings › Updates). A package manager that also
updates it does no harm, but users who prefer the package manager can set
Updates to Off; the Scoop manifest says so in its notes.
