import SwiftUI
import Observation

enum Screen: Hashable {
    case editor
    case event(Int)
    case compare(JSON?)
    case map(MapRequest)
    case runSummary
    case settings
    case customGens
}

/// What a map screen should show: the open route's game (with "add to
/// route"), or a browsed version; optionally focused on an event.
struct MapRequest: Hashable {
    var version: String? = nil
    var focusEvent: Int? = nil
}

struct Toast: Identifiable, Equatable {
    let id = UUID()
    let text: String
    let kind: String
}

@MainActor
@Observable
final class AppModel {
    var started = false
    var startError: String?
    var app: JSON = .null
    var rows: [JSON] = []
    var filtering = false
    var flattened = false
    var path: [Screen] = []
    var toast: Toast?
    var alert: String?
    /// bumped whenever the route changed, so open screens reload
    var revision = 0
    /// the event the editor last focused (insertion point for "add")
    var focusedEvent: Int?
    var settings: JSON = .null

    var isOpen: Bool { app.open.bool }
    var generation: Int { app.generation.int ?? 3 }
    var routeName: String { app.name.str }

    func start() async {
        // test hook: seed routes from a folder on the host (simulator only)
        if let seed = ProcessInfo.processInfo.environment["XPR_SEED"] {
            let dest = Core.shared.documents.appendingPathComponent("saved_routes")
            try? FileManager.default.createDirectory(at: dest, withIntermediateDirectories: true)
            for f in (try? FileManager.default.contentsOfDirectory(atPath: seed)) ?? [] where f.hasSuffix(".json") {
                let to = dest.appendingPathComponent(f)
                if !FileManager.default.fileExists(atPath: to.path) {
                    try? FileManager.default.copyItem(atPath: (seed as NSString).appendingPathComponent(f), toPath: to.path)
                }
            }
        }
        do {
            try await Core.shared.start()
            started = true
            await refreshState()
            await loadSettings()
            if await smoke() { return }
            let lib = try? await Core.shared.call("library.list")
            if lib?.auto_load.bool == true {
                if (try? await Core.shared.call("route.load_most_recent"))?.bool == true {
                    await routeChanged()
                    path = [.editor]
                }
            }
        } catch {
            startError = error.localizedDescription
        }
    }

    /// Test hooks (like the desktop's XPR_SMOKE_*): open a route and a screen
    /// at launch. `simctl launch` passes them as SIMCTL_CHILD_XPR_SMOKE_*.
    var smokeSheet: String?
    private func smoke() async -> Bool {
        let env = ProcessInfo.processInfo.environment
        guard let route = env["XPR_SMOKE_ROUTE"] else { return false }
        await openRoute(route)
        let target = env["XPR_SMOKE_PATH"] ?? "editor"
        let parts = target.split(separator: ":").map(String.init)
        let n = parts.count > 1 ? Int(parts[1]) ?? 0 : 0
        switch parts[0] {
        case "event":
            let ids = rows.filter { $0.kind.str == "group" }.compactMap { $0.id.int }
            if ids.indices.contains(n) { path.append(.event(ids[n])) }
        case "battle":
            let ids = rows.filter { $0.type.str == "Fight Trainer" }.compactMap { $0.id.int }
            if ids.indices.contains(n) { path.append(.event(ids[n])) }
        case "map": path.append(.map(MapRequest()))
        case "compare": path.append(.compare(["a": ["current": true], "b": ["name": .string(env["XPR_SMOKE_B"] ?? route)]]))
        case "run": path.append(.runSummary)
        case "settings": path.append(.settings)
        case "library": path = []
        case "new": path = []; smokeSheet = "new"
        default: smokeSheet = parts[0]
        }
        return true
    }

    /// Run a command; errors become an alert. Returns nil on failure.
    @discardableResult
    func run(_ method: String, _ params: JSON = [:], quiet: Bool = false) async -> JSON? {
        do {
            let r = try await Core.shared.call(method, params)
            await drainMessages()
            return r
        } catch {
            if !quiet { alert = error.localizedDescription }
            await drainMessages()
            return nil
        }
    }

    /// Run a route-changing command, then reload the list and state.
    @discardableResult
    func mutate(_ method: String, _ params: JSON = [:]) async -> JSON? {
        let r = await run(method, params)
        await routeChanged()
        return r
    }

    func loadSettings() async {
        if let s = try? await Core.shared.call("settings.get") { settings = s }
    }

    func refreshState() async {
        if let s = try? await Core.shared.call("app.state") { app = s }
    }

    func refreshRows() async {
        guard isOpen else { rows = []; return }
        if let r = try? await Core.shared.call("tree.rows") {
            rows = r.rows.array
            filtering = r.filtering.bool
            flattened = r.flattened.bool
        }
    }

    func routeChanged() async {
        await refreshState()
        await refreshRows()
        revision += 1
    }

    func drainMessages() async {
        guard let msgs = try? await Core.shared.call("app.messages") else { return }
        for m in msgs.array {
            let kind = m.kind.str
            if kind == "error" {
                alert = m.text.str
            } else {
                show(m.text.str, kind: kind)
            }
        }
    }

    func show(_ text: String, kind: String = "info") {
        withAnimation(.spring(duration: 0.35)) { toast = Toast(text: text, kind: kind) }
        let t = toast
        Task {
            try? await Task.sleep(for: .seconds(2.6))
            if toast == t { withAnimation(.easeOut) { toast = nil } }
        }
    }

    func openRoute(_ name: String) async {
        if await run("route.load", ["name": .string(name)]) != nil {
            focusedEvent = nil
            await routeChanged()
            path = [.editor]
        }
    }

    func save() async {
        let name = routeName.trimmingCharacters(in: .whitespaces)
        if name.isEmpty {
            alert = "Give the route a name first (tap the title)."
            return
        }
        await mutate("route.save", ["name": .string(name)])
    }

    func closeRoute() async {
        await run("route.close")
        await routeChanged()
        path = []
    }

    func undo() async {
        // undo rebuilds the tree: remembered ids are gone
        focusedEvent = nil
        await mutate("app.undo")
        Haptics.tap()
    }
}
