import CryptoKit
import Darwin
import Foundation

// 本程序是独立 API 实验，不实现产品授权、资源 UUID 或工作区关系。
struct Snapshot: Codable {
    let path: String
    let inode: String?
    let volumeIdentifier: String?
    let documentIdentifier: Int?
    let documentIdentifierAvailable: Bool
    let fileResourceIdentifier: String?
    let volumeFormat: String?
    let size: Int?
    let fixtureContentSHA256: String?
}

struct Resolution: Codable {
    let path: String?
    let stale: Bool?
    let error: String?
}

struct Observation: Codable {
    let scenario: String
    let before: Snapshot
    let expectedPath: String
    let after: Snapshot?
    let other: Snapshot?
    let resolution: Resolution
    let error: String?
}

struct PersistedCase: Codable {
    let scenario: String
    let bookmark: Data
    let expectedPath: String
}

struct Manifest: Codable {
    let schemaVersion: Int
    let fixtureRoot: String
    let cases: [PersistedCase]
}

struct RunResult: Codable {
    let mode: String
    let os: String
    let observations: [Observation]
    let manifestPath: String
}

enum ProbeError: Error, CustomStringConvertible {
    case invalid(String)
    var description: String {
        switch self { case .invalid(let reason): return reason }
    }
}

let fm = FileManager.default
let encoder = JSONEncoder()
encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]

func emit<T: Encodable>(_ value: T) throws {
    FileHandle.standardOutput.write(try encoder.encode(value))
    FileHandle.standardOutput.write(Data("\n".utf8))
}

func snapshot(_ url: URL, readFixture: Bool = false) throws -> Snapshot {
    let values = try url.resourceValues(forKeys: [
        .documentIdentifierKey, .fileResourceIdentifierKey, .volumeIdentifierKey,
        .volumeLocalizedFormatDescriptionKey, .fileSizeKey,
    ])
    let attrs = try fm.attributesOfItem(atPath: url.path)
    let digest = readFixture
        ? SHA256.hash(data: try Data(contentsOf: url)).map { String(format: "%02x", $0) }.joined()
        : nil
    return Snapshot(
        path: url.path,
        inode: attrs[.systemFileNumber].map { String(describing: $0) },
        volumeIdentifier: values.volumeIdentifier.map { String(describing: $0) },
        documentIdentifier: values.documentIdentifier,
        documentIdentifierAvailable: values.documentIdentifier != nil,
        fileResourceIdentifier: values.fileResourceIdentifier.map { String(describing: $0) },
        volumeFormat: values.volumeLocalizedFormatDescription,
        size: values.fileSize,
        fixtureContentSHA256: digest
    )
}

func makeBookmark(_ url: URL) throws -> Data {
    try url.bookmarkData(
        options: [],
        includingResourceValuesForKeys: [.documentIdentifierKey, .fileResourceIdentifierKey],
        relativeTo: nil
    )
}

func resolve(_ bookmark: Data) -> Resolution {
    do {
        var stale = false
        let url = try URL(
            resolvingBookmarkData: bookmark,
            options: [.withoutUI, .withoutMounting],
            relativeTo: nil,
            bookmarkDataIsStale: &stale
        )
        return Resolution(path: url.path, stale: stale, error: nil)
    } catch {
        return Resolution(path: nil, stale: nil, error: String(describing: error))
    }
}

func physicalPath(_ url: URL) -> String? {
    let path = url.standardizedFileURL.path
    if let resolved = realpath(path, nil) {
        defer { free(resolved) }
        return String(cString: resolved)
    }
    guard errno == ENOENT else { return nil }
    // Foundation 对存在的 /private/tmp 和未创建子项会返回不同别名；用真实父目录核对新叶子。
    var info = stat()
    guard lstat(path, &info) != 0,
          let parent = realpath(url.deletingLastPathComponent().path, nil) else { return nil }
    defer { free(parent) }
    return (String(cString: parent) as NSString).appendingPathComponent(url.lastPathComponent)
}

func contained(_ url: URL, in root: URL) -> Bool {
    guard let path = physicalPath(url), let base = physicalPath(root) else { return false }
    return path.hasPrefix(base + "/")
}

func validateTemporaryRoot(_ root: URL) throws {
    let temporaryRoots = [URL(fileURLWithPath: "/private/tmp"), fm.temporaryDirectory]
    guard temporaryRoots.contains(where: { contained(root, in: $0) }) else {
        throw ProbeError.invalid("fixture-root must be a dedicated child of /private/tmp or the OS temporary directory")
    }
}

func prepareRoot(_ path: String) throws -> URL {
    let root = URL(fileURLWithPath: path).standardizedFileURL
    try validateTemporaryRoot(root)
    var existing = stat()
    guard lstat(root.path, &existing) != 0, errno == ENOENT else {
        throw ProbeError.invalid("fixture-root already exists; choose a new empty experiment path, existing data is never overwritten")
    }
    try fm.createDirectory(at: root, withIntermediateDirectories: true)
    try Data("worktwill-native-fixture-v1\n".utf8).write(to: root.appendingPathComponent(".worktwill-fixture-root"))
    return root
}

func runFixtures(_ root: URL) throws {
    let scenarios = ["in_place", "rename", "same_volume_move", "atomic_write", "replace_item", "copy", "delete_recreate"]
    var observations: [Observation] = []
    var persisted: [PersistedCase] = []
    for scenario in scenarios {
        let directory = root.appendingPathComponent(scenario, isDirectory: true)
        try fm.createDirectory(at: directory, withIntermediateDirectories: false)
        let original = directory.appendingPathComponent("contract.txt")
        try Data("fixture-original-document\n".utf8).write(to: original)
        let before = try snapshot(original, readFixture: true)
        let bookmark = try makeBookmark(original)
        var target = original
        var other: Snapshot?
        var mutationError: String?
        do {
            switch scenario {
            case "in_place":
                try Data("fixture-edited-in-place\n".utf8).write(to: original)
            case "rename":
                target = directory.appendingPathComponent("renamed-contract.txt")
                try fm.moveItem(at: original, to: target)
            case "same_volume_move":
                let nested = directory.appendingPathComponent("nested", isDirectory: true)
                try fm.createDirectory(at: nested, withIntermediateDirectories: false)
                target = nested.appendingPathComponent("contract.txt")
                try fm.moveItem(at: original, to: target)
            case "atomic_write":
                try Data("fixture-atomic-save\n".utf8).write(to: original, options: .atomic)
            case "replace_item":
                let replacement = directory.appendingPathComponent("replacement.txt")
                try Data("fixture-replacement-document\n".utf8).write(to: replacement)
                _ = try fm.replaceItemAt(original, withItemAt: replacement)
            case "copy":
                let copy = directory.appendingPathComponent("copy.txt")
                try fm.copyItem(at: original, to: copy)
                other = try snapshot(copy, readFixture: true)
            case "delete_recreate":
                // 只删除本次刚生成的 fixture，不接受任意文件操作参数。
                try fm.removeItem(at: original)
                try Data("fixture-unrelated-new-document\n".utf8).write(to: original)
            default:
                throw ProbeError.invalid("unknown built-in scenario")
            }
        } catch {
            mutationError = String(describing: error)
        }
        var after: Snapshot?
        do { after = try snapshot(target, readFixture: true) }
        catch { mutationError = (mutationError.map { $0 + "; " } ?? "") + "snapshot: \(error)" }
        observations.append(Observation(
            scenario: scenario, before: before, expectedPath: target.path,
            after: after, other: other, resolution: resolve(bookmark), error: mutationError
        ))
        persisted.append(PersistedCase(scenario: scenario, bookmark: bookmark, expectedPath: target.path))
    }
    let manifestURL = root.appendingPathComponent("manifest.json")
    let manifest = Manifest(schemaVersion: 1, fixtureRoot: root.path, cases: persisted)
    try encoder.encode(manifest).write(to: manifestURL)
    try emit(RunResult(
        mode: "native_api_fixture_observations_not_office_acceptance",
        os: ProcessInfo.processInfo.operatingSystemVersionString,
        observations: observations, manifestPath: manifestURL.path
    ))
}

func resume(_ path: String) throws {
    let manifestURL = URL(fileURLWithPath: path).standardizedFileURL
    let manifest = try JSONDecoder().decode(Manifest.self, from: Data(contentsOf: manifestURL))
    let root = URL(fileURLWithPath: manifest.fixtureRoot)
    try validateTemporaryRoot(root)
    let marker = root.appendingPathComponent(".worktwill-fixture-root")
    guard manifest.schemaVersion == 1, contained(manifestURL, in: root),
          try Data(contentsOf: marker) == Data("worktwill-native-fixture-v1\n".utf8) else {
        throw ProbeError.invalid("not a recognized native fixture manifest")
    }
    struct Resumed: Codable {
        let scenario: String
        let resolution: Resolution
        let expectedPathMatches: Bool
        let snapshot: Snapshot?
        let error: String?
    }
    var output: [Resumed] = []
    for item in manifest.cases {
        let result = resolve(item.bookmark)
        var observed: Snapshot?
        var error: String?
        if let target = result.path {
            let url = URL(fileURLWithPath: target)
            guard contained(url, in: root) else {
                throw ProbeError.invalid("resolved target escaped fixture-root")
            }
            do { observed = try snapshot(url, readFixture: true) }
            catch let caught { error = String(describing: caught) }
        }
        output.append(Resumed(
            scenario: item.scenario, resolution: result,
            expectedPathMatches: result.path == item.expectedPath,
            snapshot: observed, error: error
        ))
    }
    try emit(output)
}

do {
    let args = Array(CommandLine.arguments.dropFirst())
    guard args.count == 2 else {
        throw ProbeError.invalid("usage: macos-identity --inspect PATH | --fixture-root NEW_TEMP_PATH | --resume MANIFEST")
    }
    switch args[0] {
    case "--inspect":
        let url = URL(fileURLWithPath: args[1]).standardizedFileURL
        struct Inspection: Codable {
            let mode: String
            let beforeBookmark: Snapshot
            let afterBookmark: Snapshot
            let bookmarkResolution: Resolution
        }
        let before = try snapshot(url)
        let bookmark = try makeBookmark(url)
        var refreshed = URL(fileURLWithPath: url.path)
        refreshed.removeAllCachedResourceValues()
        try emit(Inspection(
            mode: "read_only_metadata_and_bookmark_no_fixture_mutation",
            beforeBookmark: before, afterBookmark: try snapshot(refreshed),
            bookmarkResolution: resolve(bookmark)
        ))
    case "--fixture-root":
        try runFixtures(prepareRoot(args[1]))
    case "--resume":
        try resume(args[1])
    default:
        throw ProbeError.invalid("unknown mode")
    }
} catch {
    FileHandle.standardError.write(Data("\(error)\n".utf8))
    exit(1)
}
