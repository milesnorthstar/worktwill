# Version iteration and release workflow

Use this document to carry a scoped fix or feature through maintenance and release. Build commands, signing, installation, CI, and update behavior remain in [INSTALL_AND_UPDATES.md](INSTALL_AND_UPDATES.md); do not maintain a second packaging procedure here.

## Publication model and current implementation

The adopted workflow is: **prepare candidates → validate the exact artifacts → stage uploads → explicitly publish → verify online → follow up**. Build a candidate once and promote those same bytes. Website content can be deployed independently, using the currently published application records.

This is the operating contract for future automation, not a claim that a unified release pipeline already exists:

| Capability | Current implementation | Required control |
| --- | --- | --- |
| Native packages and update signatures | `pnpm release:build` builds on the native platform with the existing signing key | Preserve artifact identity from build through validation and publication |
| CI candidates | The manual Windows workflow runs checks and packaging; Actions artifacts expire after 14 days | Add the required macOS job and coordinate the requested platform candidates; no cloud run is established by this document |
| Candidate registration | `release:record` immediately edits `website/release.json`; its candidate `stage` is display text | Separate candidate metadata from promotion into the active release registry |
| Website and update manifests | Website preparation checks local artifact size/hash and generates manifests from the registry | Protect website-only deployments from accidentally promoting a candidate; restore exact published artifacts on clean runners |
| Upload, release archive, and online checks | No unified GitHub Release upload/promotion/verification command exists | Automate exact-artifact upload, permanent version archive, remote checks, and serialized promotion |
| Withdrawal | No dedicated safe withdrawal command exists | Verify removal of affected generated download/update entries without recalling cached packages or downgrading user data |

Until candidate isolation is implemented, do not use `release:record` as an ordinary candidate inventory command in a checkout used for website deployment. Stage a registry change only as part of the authorized release; review it before deployment. The `stage` text, a local registry timestamp, and a successful build are not release gates.

## Authorization and release handoff

A release task identifies the version, platform/architecture targets, source commit or agreed scope, release notes, and validation needed. A request such as “publish version X for macOS ARM64 and Windows x64” authorizes the defined build, validation, upload, release/tag, website/update publication, and online verification steps using the existing project resources. Do not request confirmation again for each of those steps. If the task names only a candidate or source push, stop at that scope.

Missing credentials, unavailable required native validation, a new compatibility decision, or changed release scope must be reported at the affected step. Continue independent preparation where useful; do not silently drop a requested platform or mark the overall release complete. New services/accounts, paid resources, DNS changes, or key rotation are separate decisions. This workflow does not itself authorize a release.

Use the existing issue/task and CI run to track the release. Record the exact candidate and per-platform state; no second task system is needed. A handoff must let another Codex run determine whether work stopped at build, validation, upload, publication, or online verification.

## 1. Scope the iteration

Start from the current issue or maintainer task: define the user-visible problem, expected result, affected platforms, and completion evidence. Review the [product contract](PRODUCT.md) and affected [architecture](../ARCHITECTURE.md) boundaries. Use the existing issue/task for progress; do not create a parallel task system or resume a dormant internal handoff.

Implement a focused change with necessary regressions. Update its owning documentation and add a concise user-facing note under `Unreleased` in [CHANGELOG.md](../CHANGELOG.md) when behavior changes. Include upgrade requirements or known limitations when they affect users; keep raw acceptance logs private. Documentation-only or website-only changes do not require a desktop version bump.

## 2. Choose the version and assess compatibility

Use semantic versions for the desktop application: patch for backward-compatible fixes, minor for backward-compatible features, major for incompatible product or integration contracts. Discuss compatibility and recovery before an incompatible change. A schema migration alone does not determine the application major version; its upgrade/downgrade effects still need review and tests.

Prepare the version once the intended release scope is clear. Keep these current application/crate versions aligned:

- `package.json` → `version`;
- `src-tauri/tauri.conf.json` → `version`;
- `src-tauri/Cargo.toml` → package version;
- `crates/worktwill-core/Cargo.toml` → package version;
- corresponding local-package entries in `Cargo.lock`, refreshed by Cargo without unrelated dependency upgrades.

The release script currently checks the first two values; the four-way check is a maintainer responsibility, not an existing automated guarantee. Application identity/data paths and the existing update-signing public key must remain stable across ordinary upgrades. Use a higher version for a new public update; rebuilding the same version will not update installed clients.

| Value or record | Authority | Meaning |
| --- | --- | --- |
| Application version | Files listed above | Version being built; not evidence it is published |
| Database schema | `crates/worktwill-core/src/migrations.rs` | Storage compatibility; version independently of the application |
| Per-platform release | `website/release.json`, written by `release:record` | Exact installer/update artifacts, version, size, hash, signature, and stage; platforms may advance separately |
| User-facing history | `CHANGELOG.md` | Reviewed changes and limitations, not build or acceptance logs |

Do not set a platform's record to a new version until that platform has the corresponding verified artifacts. A local registry entry or its timestamp does not establish online availability.

## 3. Validate the candidate

Keep a candidate manifest with the build artifacts, separate from the active `website/release.json`. This manifest is required metadata for the proposed pipeline, not an existing generated file format. It contains:

- exact source commit and clean/dirty state, application version, platform/architecture, build environment, and CI run or local build identifier;
- each artifact's role, filename, byte count, SHA-256, and update-signature information;
- relevant validation results and platform/editor versions, known limitations, and sanitized evidence references.

Publish only traceable source commits; do not tag or promote an artifact built from uncommitted runtime changes. Candidate validation and promotion bind to this exact manifest. Rebuilding produces a new candidate that needs its own relevant validation; do not carry a pass to different bytes.

Run the checks required by [CONTRIBUTING.md](../CONTRIBUTING.md) and native checks relevant to this change on each affected platform. For migrations, test existing-data upgrades, failure preservation, and restore permissions. For updater changes, test pending saves, version/signature handling, upgrade, restart, and retained data.

Before promotion, verify update signatures against the configured public key and the announced version, not just the existence of a `.sig` file or its decoded version comment. This cryptographic preflight is a requirement to implement; the current record script only checks the version declaration, and client-side verification does not prove the publication preflight exists.

Mark each check passed, failed, blocked, or not run; distinguish local fixtures, CI packaging, native behavior, and online download/update. An unavailable environment is a gap to report, not permission to claim support or rerun unrelated checks indefinitely.

## 4. Package and publish

With release authorization, follow the existing [packaging and registration procedure](INSTALL_AND_UPDATES.md). Apply these stages per target; a later failure does not turn earlier stages into a completed release:

| Stage | Action and exit condition |
| --- | --- |
| Candidate prepared | Native installer, update package/signature, installation help, checksums, and candidate manifest exist |
| Candidate validated | Required checks pass for the exact artifact; missing evidence is resolved or an explicit scope decision is recorded |
| Uploads staged | The exact artifacts are uploaded or included in the checked deployment bundle; archive uploads are checked within their current access scope and external distribution URLs return the expected bytes |
| Published | The authorized platform records, download pages, update manifests, release notes, and version archive are exposed consistently |
| Online verified | Actual public pages, installer/update bytes, manifest versions/signatures/URLs, and applicable upgrade behavior are checked; remaining gaps are stated |

### Artifact storage and promotion

- Use Actions artifacts for transfer, not permanent version history. The target archive is a versioned GitHub Release containing installers, update packages/signatures, checksums, installation help, and sanitized provenance. In the proposed automation, uploads can be staged on a draft release before publication; do not publish a draft as a side effect of building.
- Keep the existing website distribution for packages within its current file-size limit. Use the same bytes as the version archive. Introduce external download storage only when needed; its URL mode already exists, but automatic provisioning/upload is not implemented.
- For externally hosted packages, verify remote size and SHA-256 before exposing their active download/update links. For website-bundled packages, check the bundle locally, deploy it together with its pages/manifests, then verify the live bytes. Do not claim remote availability before deployment.
- Before promotion, recheck the active platform version and candidate provenance. Serialize publication for the same channel, prevent an older/stale release task from replacing a newer record, and retain the previous published records for recovery. These are required automation controls, not current script guarantees.
- Change only the approved target entries in `website/release.json`. Keep other platforms' published records and artifacts intact. Publish that reviewed registry snapshot, not an unrelated checkout's latest state.
- Use isolated deployment output and inspect it for stale or unintended download/update entries. The current generator writes entries but does not implement safe withdrawal cleanup. Never replace an existing artifact's bytes under the same URL, reuse a published tag for different source, or rotate keys to fix packaging.

### Version notes and completion

Move only the shipped entries from `Unreleased` into a dated version section when publishing, with explicit platform scope and remaining limitations. If platforms ship separately, state which are available; do not declare the other platform released. Keep any GitHub Release notes consistent with this entry and the registry. Do not invent historical release entries from candidate versions or test reports.

Source commit/push, tags/GitHub Releases, artifact uploads, and website/update publication are distinct actions. Perform those requested by the task and report which actually completed. Use `v<version>` for an authorized release tag and bind it to the candidate source commit; do not move published tags. Authorization to fix or push source alone does not authorize distribution.

Release closeout includes the source/tag, each platform's candidate and version, checksums, archive/download/update URLs, publication and verification results, and known limitations. An uploaded package with a failed deployment is “uploaded, not published”; a deployment without completed online checks is “published, verification incomplete.” A partial failure must remain visible and retries must resume from the verified stage without rebuilding or overwriting artifacts unnecessarily.

## 5. Synchronize the website and follow up

For website-only work, preserve the currently published per-platform records and package bytes. On a clean runner, the target workflow retrieves published artifacts from the version archive or approved distribution source and verifies them against the registry; it must not depend on a developer's ignored `deliverables/`. Retrieval automation is not implemented yet. Until it exists, supply and verify those exact files explicitly, and compare the intended registry with the live published state before deployment. Do not delete records, rebuild packages, or bump the desktop version merely to make a content build succeed.

When shipped behavior, support, privacy, or install steps change, update both website languages and the relevant product, guide, download, or privacy pages from their source templates. Review [SEO/GEO maintenance](../website/geo/README.md): metadata, routes/redirects, structured facts, dates, sitemap, and public support claims must agree. Do not announce an unshipped feature through SEO content.

Verify the actual public pages, installer bytes, and update manifests after an authorized deployment. Track reported regressions through existing issues. Compare search/index/citation observations in matching windows; do not confuse a successful HTTP audit with discovery or adoption.

## 6. Withdraw a release and recover

An explicit withdrawal task identifies the affected version, targets, download/update exposure, and user impact. Stop the relevant pending publication tasks. Prepare a checked replacement website/manifest snapshot that stops advertising or offering the faulty version; verify the actual generated output and public responses. Removing a registry entry alone is insufficient if generated files remain. Preserve unaffected platform records and do not treat an HTTP error as a completed withdrawal mechanism.

Mark the affected release clearly and remove unsafe public downloads as required by the task, while retaining exact faulty bytes and failure evidence in private storage. Preserve commit/tag provenance; do not overwrite the original version with a repaired package. State what has actually been withdrawn and where it may still be available.

Withdrawal cannot guarantee recall of already downloaded or cached packages, undo an installation, or reverse a database migration. Do not promise automatic client recall or downgrade. Prefer a verified higher-version patch. If rollback is necessary, assess the matching binary, schema and data snapshot, authorization, and recovery evidence before executing it; an older installer alone is not a recovery plan.

## Automation follow-up

Implementation order: isolate candidate registration from stable promotion → coordinate required native build jobs and provenance → implement archive/upload/signature and remote-byte verification → guard website-only deployments and serialized publication → implement and test withdrawal cleanup. Reuse existing scripts and registry ownership. Do not invent executable commands or claim these controls are active before implementation and acceptance.
