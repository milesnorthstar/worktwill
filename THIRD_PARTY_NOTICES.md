# Third-party licensing / 第三方许可说明

WorkTwill's own code is licensed under the [MIT License](LICENSE). Third-party code, libraries, tools, and assets retain their respective licenses and copyright notices. The project license does not relicense them.

WorkTwill 自有代码采用 MIT；第三方代码、依赖、工具与素材保留各自的许可和版权。本文件是许可核查范围说明，**不是安装包中全部第三方许可文本的汇编，也不证明发行许可审计已完成**。

## Source of license information / 许可信息来源

- Node dependencies are resolved by `pnpm-lock.yaml`. Consult each package's `package.json` and its shipped `LICENSE`, `COPYING`, or notice files. Runtime dependencies include React, React DOM, Lucide React, and the Tauri JavaScript API; the website also uses Lucide.
- Rust dependencies are resolved by `Cargo.lock`. Consult each crate's `Cargo.toml` and included license and notice files. The desktop runtime includes Tauri, its plugins, and the core's dependencies, including bundled SQLite.
- A dependency's metadata is an index, not a substitute for its license text. Bundled native libraries and embedded data can have additional notices.

Node 与 Rust 锁文件用于定位实际版本；应保留依赖随附的许可文本及通知，不能只依据包元数据中的许可证名称。原生库、内嵌数据和安装器组件也需按实际发行内容核查。

## Review scope / 当前核查范围

2026-11-22: reviewed license metadata for locally installed Node packages and locally cached registry crates matching `Cargo.lock`. This was a metadata review, not a complete source-provenance or binary-distribution audit.

- Cached Rust declarations include MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0, and MPL-2.0, including dual-licensed combinations. MPL-covered files retain their own terms; the project's MIT license does not replace them.
- The installed Node dependency tree includes an LGPL-3.0-or-later libvips binary in `@img/sharp-libvips-darwin-arm64`. This is present in the local tooling tree; its presence alone does not establish that it is shipped in the desktop app. If redistributed, its LGPL obligations must be handled separately.
- The locally installed legacy `xtend` 2.1.2 package under a `cf` tooling tree has no `license` field. Its actual license files and provenance require separate verification if redistributed.
- 220 registry packages in the cross-platform Rust lockfile were not cached locally and were not reviewed. No missing packages were downloaded for this review.
- Repository assets and copied source require provenance review beyond dependency metadata. No complete asset-provenance audit is claimed.

本轮未发现仅凭已读取的许可名称就必须阻止自有代码采用 MIT 的事项；但未缓存依赖、第三方源文件、素材来源及安装包随附声明仍需按实际发行内容核查，不能据此宣称全部依赖或所有平台安装包已经满足分发义务。

## Before distributing installers / 安装包发行要求

The 1.3.2 macOS release build uses `scripts/distribution-notices.mjs` to collect actual target-platform Rust dependencies, frontend runtime packages, and Rust standard-library notices. The application bundle includes `Contents/Resources/notices/THIRD-PARTY.txt`, `components.json`, the WorkTwill license, and Rust library notices. This is a conservative dependency closure that also includes supporting macros; it is not a claim that every listed package contributes machine code. Unmodified MPL-covered crate sources are linked by exact version. Missing upstream license files are supplemented by the versioned sources and explanations in `scripts/license-sources/sources.json`; missing texts fail the release build.

1.3.2 的 macOS 发行包在完整应用签名前生成并嵌入上述声明。构建后需检查实际 DMG 与更新归档中的许可文件及签名，不能只检查源码列表。其他平台和旧版本仍需独立核查，不能沿用本次 Mac 结论。

Windows notice generation also supplements the omitted upstream MIT text for `webview2-com`, `webview2-com-sys`, and `webview2-com-macros` through `scripts/license-sources/sources.json`. Each entry identifies the source commit recorded by its published crate. Successful generation does not establish that an existing Windows installer contains these notices; inspect a newly built candidate separately.

For each platform and version, identify the third-party components actually shipped, include their required copyright notices and license texts, and satisfy any applicable source-availability or other obligations. Preserve required upstream notices when modifying third-party code. Keep build-only tools separate from distributed runtime components.

各平台安装包需携带实际分发组件要求的版权及完整许可文本，并按相应条款处理源码获取等义务。构建工具不能未经区分直接等同于应用运行时依赖。本次添加 MIT 许可证没有重新构建安装包，也没有将本说明自动嵌入既有安装包。
