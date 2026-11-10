import SwiftUI

/// App settings: battle calculation, battle summary display, route list
/// colours, custom games and storage.
struct SettingsView: View {
    @Environment(AppModel.self) private var model
    var body: some View {
        Form {
            BattleConfigSections()
            Section("Battle summary") {
                toggle("Move highlights", "show_move_highlights")
                toggle("Fade moves without a highlight", "fade_moves").disabled(!model.settings.show_move_highlights.bool)
                toggle("Test moves", "test_moves")
                toggle("Setup / weather / transform controls", "show_legacy")
                toggle("Open the battle summary first", "auto_switch")
            }
            Section("Route list") {
                toggle("Colour major battles", "color_major_battles")
                toggle("Fade folder names", "fade_folder_text")
                NavigationLink { HighlightColorsView() } label: { Label("Highlight colours", systemImage: "paintpalette") }
            }
            Section("Routes") {
                toggle("Open the most recent route at launch", "auto_load")
                NavigationLink { CustomGensView() } label: { Label("Custom games", systemImage: "gamecontroller") }
            }
            Section {
                LabeledContent("Version", value: model.settings.app_version.str)
                LabeledContent("Routes folder") {
                    Text("Files ▸ On My iPhone ▸ XP Router").font(.caption).foregroundStyle(.secondary)
                }
            } header: { Text("About") } footer: {
                Text("Saved routes are plain JSON files, the same format as the desktop app: share them with AirDrop, Mail or Files, or open one in this app to import it.")
            }
        }
        .navigationTitle("Settings")
        .task { await model.loadSettings() }
    }

    private func toggle(_ label: String, _ key: String) -> some View {
        Toggle(label, isOn: Binding(get: { model.settings[key].bool }, set: { v in
            Task { if let s = await model.run("settings.set", ["key": .string(key), "value": .bool(v)]) { model.settings = s } }
        }))
    }
}

/// The desktop's Battle Configuration dialog.
struct BattleConfigSections: View {
    @Environment(AppModel.self) private var model
    var body: some View {
        Section {
            stepper("Search depth (turns)", "damage_search_depth", 1...99)
            Toggle("Ignore accuracy in kill ranges", isOn: bind("ignore_accuracy"))
            Toggle("Fully calculate Psywave", isOn: bind("force_full_search"))
        } header: { Text("Damage calculation") } footer: { Text("Kill ranges list up to three lines; chances below 0.1% are dropped. Full Psywave calculation is slow and rarely needed.") }
        Section {
            strategy("Your best move", "player_strategy")
            strategy("Enemy's best move", "enemy_strategy")
            stepper("Consistency threshold (%)", "consistent_threshold", 1...99)
        } header: { Text("Best-move highlight") } footer: {
            Text("Guaranteed Kill: fewest turns to a sure kill. Fastest Kill: the first kill chance. Consistent Kill: the first range at least the threshold likely. Ties go to fewer turns, then accuracy, then avoiding 2-turn moves, then max damage; Hyper Beam only wins when it kills.")
        }
    }

    private func bind(_ key: String) -> Binding<Bool> {
        Binding(get: { model.settings[key].bool }, set: { v in set(key, .bool(v)) })
    }

    private func stepper(_ label: String, _ key: String, _ range: ClosedRange<Int>) -> some View {
        Stepper(value: Binding(get: { model.settings[key].int ?? range.lowerBound }, set: { set(key, .number(Double($0))) }), in: range) {
            HStack { Text(label); Spacer(); Text("\(model.settings[key].int ?? 0)").monospacedDigit().foregroundStyle(.secondary) }
        }
    }

    private func strategy(_ label: String, _ key: String) -> some View {
        Picker(label, selection: Binding(get: { model.settings[key].str }, set: { set(key, .string($0)) })) {
            ForEach(model.settings.strategies.strings, id: \.self) { Text($0).tag($0) }
        }
    }

    private func set(_ key: String, _ v: JSON) {
        Task { if let s = await model.run("settings.set", ["key": .string(key), "value": v]) { model.settings = s } }
    }
}

struct BattleConfigSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(AppModel.self) private var model
    var body: some View {
        NavigationStack {
            Form { BattleConfigSections() }
                .navigationTitle("Battle Configuration")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
                .task { await model.loadSettings() }
        }
    }
}

struct HighlightColorsView: View {
    @Environment(AppModel.self) private var model
    var body: some View {
        Form {
            Section("Highlights") {
                ForEach(0..<9, id: \.self) { i in
                    ColorPicker("Highlight \(i + 1)", selection: Binding(get: { Color(hex: model.settings.highlight_colors[i].str) }, set: { c in
                        set(["key": "highlight_color", "index": .number(Double(i + 1)), "value": .string(c.hexString)])
                    }), supportsOpacity: false)
                }
            }
            Section("Major battles") {
                ForEach(model.settings.category_colors.array.map { Item($0, key: "key") }) { c in
                    ColorPicker(c["label"].str, selection: Binding(get: { Color(hex: c["color"].str) }, set: { v in
                        set(["key": "category_color", "category": c["key"], "value": .string(v.hexString)])
                    }), supportsOpacity: false)
                }
                .disabled(!model.settings.color_major_battles.bool)
            }
            Section {
                Button("Reset to Defaults", role: .destructive) { set(["key": "reset_highlight_colors"]) }
            }
        }
        .navigationTitle("Highlight Colours")
    }

    private func set(_ p: JSON) {
        Task {
            if let s = await model.run("settings.set", p) { model.settings = s }
            await model.refreshRows()
        }
    }
}

/// Custom games: copies of a game's data you can edit (in Files ▸ XP Router
/// ▸ custom_gens) to route romhacks.
struct CustomGensView: View {
    @Environment(AppModel.self) private var model
    @State private var data: JSON = .null
    @State private var base = "Yellow"
    @State private var name = ""
    @State private var backport: BackportJob?
    var body: some View {
        Form {
            Section {
                Picker("Based on", selection: $base) {
                    ForEach(data.bases.strings, id: \.self) { Text($0).tag($0) }
                }
                TextField("New game name", text: $name)
                Button("Create Custom Game") {
                    Task {
                        if await model.run("customgen.create", ["base": .string(base), "name": .string(name)]) != nil {
                            model.show("Created \(name)", kind: "saved")
                            name = ""
                            await load()
                        }
                    }
                }
                .disabled(name.trimmingCharacters(in: .whitespaces).isEmpty)
            } header: { Text("New custom game") } footer: {
                Text("The game's data files are copied to Files ▸ On My iPhone ▸ XP Router ▸ custom_gens, where you (or the desktop app) can edit them.")
            }
            Section("Custom games") {
                if data.customs.array.isEmpty { Text("None yet").foregroundStyle(.secondary) }
                ForEach(data.customs.array.map { Item($0, key: "name") }) { c in
                    LabeledContent(c["name"].str, value: c["base"].str)
                        .contextMenu {
                            Button { backport = BackportJob(game: c["name"].str) } label: { Label("Import Backport…", systemImage: "square.and.arrow.down") }
                        }
                        .swipeActions {
                            Button { backport = BackportJob(game: c["name"].str) } label: { Label("Backport", systemImage: "square.and.arrow.down") }.tint(.indigo)
                        }
                }
                Button("Reload from Files") { Task { await model.run("customgen.reload"); await load() } }
            }
        }
        .navigationTitle("Custom Games")
        .task { await load() }
        .sheet(item: $backport) { job in BackportSheet(job: job) }
    }
    private func load() async { data = (await model.run("customgen.list", quiet: true)) ?? .null }
}

struct BackportJob: Identifiable {
    let game: String
    var id: String { game }
}

/// Import Backport: a romhack's backports.asm (and optionally
/// backport_moves.asm) adds a species to a custom game.
struct BackportSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let job: BackportJob
    @State private var asm: URL?
    @State private var moves: URL?
    @State private var species = ""
    @State private var picking: Int?
    @State private var result: JSON = .null

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Button { picking = 0 } label: { LabeledContent("backports.asm", value: asm?.lastPathComponent ?? "Choose") }
                    Button { picking = 1 } label: { LabeledContent("backport_moves.asm", value: moves?.lastPathComponent ?? "Optional") }
                    TextField("Species name", text: $species)
                } footer: { Text("Adds the backported Pokémon (and its new moves) to \(job.game).") }
                if let text = result.text.string {
                    Section("Result") {
                        Text(text).font(.footnote).textSelection(.enabled)
                        Button("Create Route for \(result.species.str)") {
                            Task {
                                if await model.run("newroute.create", ["version": result.version, "species": result.species, "base": "Empty Route", "name": .string("\(job.game.lowercased())-\(result.species.str.lowercased())")]) != nil {
                                    await model.routeChanged()
                                    dismiss()
                                    model.path = [.editor]
                                }
                            }
                        }
                    }
                }
            }
            .tint(.primary)
            .navigationTitle("Import Backport")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Import") { Task { await run() } }.disabled(asm == nil || species.isEmpty)
                }
            }
            .fileImporter(isPresented: Binding(get: { picking != nil }, set: { if !$0 { picking = nil } }), allowedContentTypes: [.item]) { r in
                guard case .success(let url) = r else { return }
                let copy = localCopy(url)
                if picking == 0 { asm = copy } else { moves = copy }
            }
        }
    }

    /// Files from other providers are only readable while their scope is
    /// open: copy them somewhere the core can read.
    private func localCopy(_ url: URL) -> URL? {
        let ok = url.startAccessingSecurityScopedResource()
        defer { if ok { url.stopAccessingSecurityScopedResource() } }
        let dest = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + "-" + url.lastPathComponent)
        return (try? FileManager.default.copyItem(at: url, to: dest)) != nil ? dest : nil
    }

    private func run() async {
        guard let asm else { return }
        if let r = await model.run("customgen.import_backport", ["name": .string(job.game), "asm": .string(asm.path), "moves": .string(moves?.path ?? ""), "species": .string(species)]) {
            result = r
            Haptics.success()
        }
    }
}
