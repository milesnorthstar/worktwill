# Changelog

User-facing changes are collected under `Unreleased` and moved to dated version sections when published. Release entries identify the available platforms and relevant upgrade limitations. This file starts with the current maintenance workflow; it does not reconstruct historical releases from internal candidates.

## Unreleased

### Fixed

- Handle both LF and CRLF source files in isolated website audit tests.

### Maintenance

- Separate prepublication checks from post-publication in-app upgrade acceptance, avoiding a circular requirement on an update feed that is not published yet while retaining observed-failure blocking.

- Isolate release candidates from public download/update records, bind build provenance and native Agent acceptance to exact artifacts, verify updater signatures before promotion, and add checked draft archiving and all-platform online byte verification commands.
- Add a manual macOS ARM64 candidate workflow and extend Windows candidate artifacts with provenance and an unpassed acceptance template. Publication and native upgrade acceptance remain separate operations.

- Add secret-free macOS and Windows checks for pull requests and main-branch pushes, including isolated website HTTP audit tests.
- Add bilingual bug/feature forms, a pull request checklist, private security reporting guidance, and weekly grouped dependency update configuration.

### Documentation

- Explain Mac first-launch approval beside website downloads and in a bilingual step-by-step guide, with separate help for missing approval buttons, damaged apps, and malware warnings.
- Organize agent guidance, product behavior, architecture, website SEO/GEO maintenance, and version iteration into focused maintenance documents.
- Define candidate promotion, artifact provenance and archiving, upload/publication verification, website-only deployment, and release withdrawal workflows, with current automation gaps stated explicitly.

## 1.3.4 — 2026-11-24

Available for macOS Apple Silicon. The exact installer passed native installation, launch, changed-interaction and shipped-license checks. Windows remains on 1.3.3; its 1.3.4 candidate awaits native acceptance and is not published. Installation and live in-app upgrade acceptance are recorded separately. OS trust signing and notarization are not included; update packages use the existing signature verification.

### Fixed

- Fit the initial desktop window to the monitor work area with a margin and center it, keeping the title bar and bottom content clear of the Windows taskbar.
- Preserve workspace counts, resource rows, selection and scroll during ordinary refreshes; coalesce operation notifications and refresh the loaded range without clearing the interface.
- Keep selection actions outside list layout and retain search results after adding files to a workspace, marking existing references as added.
- Offer per-row removal from a workspace without selecting first, while preserving original files, favorites and references in other workspaces.

## 1.3.3 — 2026-11-23

Available for Windows x64 as a GitHub prerelease. Native installation, launch, snapshot backup/restore and packaged license notices were verified on the exact installer. The official website also distributes this installer; macOS remains on 1.3.2. Online upgrade from a lower Windows version has not been verified.

### Distribution

- Restore Windows distribution-notice generation by supplying the exact upstream WebView2 binding license texts omitted from the published crates.
- Include target-platform third-party license texts, component versions, MPL source-download links, and Rust standard-library notices with the Windows installation.

## 1.3.2 — 2026-11-23

Available for macOS Apple Silicon. Windows is not updated by this release. Apple Developer ID signing and notarization are not included; first launch can still require **Open Anyway** in Privacy & Security. Live in-app upgrade acceptance is tracked separately from installation and online artifact checks.

### Fixed

- Fix incomplete macOS application signatures that could cause a downloaded app to be reported as damaged. Sign the complete bundle before packaging and reject invalid application and update-archive signatures during release builds.
- Fail snapshot copying promptly on database lock contention instead of waiting in SQLite's busy handler while holding the application data gate; preserve the existing explicit-retry and data-preservation behavior.

### Distribution

- Include target-platform third-party license texts, component versions, MPL source-download links, and Rust standard-library notices inside the application bundle.
