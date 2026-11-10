import SwiftUI
import UniformTypeIdentifiers

/// The landing page: the saved routes, the open route, and the ways in
/// (new route, compare, map browser, import, settings).
struct LibraryView: View {
    @Environment(AppModel.self) private var model
    @State private var lib: JSON = .null
    @State private var search = ""
    @State private var showNew = false
    @State private var importing = false
    @State private var renaming: String?
    @State private var renameText = ""
    @State private var deleting: String?
    @State private var pendingOpen: String?
    @State private var shareURL: URL?

    private var sort: String { lib.sort.string ?? "most_recent" }
    private var routes: [JSON] { lib.routes.array }

    var body: some View {
        List {
            if model.isOpen {
                Section {
                    OpenRouteCard()
                        .listRowInsets(EdgeInsets())
                        .listRowBackground(Color.clear)
                }
            }
            Section {
                QuickActions(showNew: $showNew, importing: $importing)
                    .listRowInsets(EdgeInsets())
                    .listRowBackground(Color.clear)
            }
            Section {
                if routes.isEmpty {
                    ContentUnavailableView {
                        Label(search.isEmpty ? "No saved routes" : "No matches", systemImage: "map")
                    } description: {
                        Text(search.isEmpty ? "Create a route, or import one made on the desktop app (Files, AirDrop or Mail)." : "Try a different search.")
                    }
                    .listRowBackground(Color.clear)
                }
                ForEach(routes.map { Item($0, key: "name") }) { r in
                    Button {
                        open(r["name"].str)
                    } label: {
                        RouteRow(route: r.json, isCurrent: lib.current.string == r["name"].str && model.isOpen)
                    }
                    .buttonStyle(.plain)
                    .swipeActions(edge: .trailing) {
                        Button(role: .destructive) { deleting = r["name"].str } label: { Label("Delete", systemImage: "trash") }
                        Button { renameText = r["name"].str; renaming = r["name"].str } label: { Label("Rename", systemImage: "pencil") }.tint(.indigo)
                    }
                    .swipeActions(edge: .leading) {
                        Button { Task { await share(r["name"].str) } } label: { Label("Share", systemImage: "square.and.arrow.up") }.tint(.blue)
                    }
                    .contextMenu {
                        Button { open(r["name"].str) } label: { Label("Open", systemImage: "arrow.up.forward.app") }
                        Button { Task { await share(r["name"].str) } } label: { Label("Share Route File", systemImage: "square.and.arrow.up") }
                        Button { model.path.append(.compare(["a": ["name": .string(r["name"].str)]])) } label: { Label("Compare…", systemImage: "rectangle.split.2x1") }
                        Button { Task { await model.run("route.duplicate", ["name": .string(r["name"].str)]); await reload() } } label: { Label("Duplicate", systemImage: "plus.square.on.square") }
                        Button { renameText = r["name"].str; renaming = r["name"].str } label: { Label("Rename", systemImage: "pencil") }
                        Divider()
                        Button(role: .destructive) { deleting = r["name"].str } label: { Label("Delete", systemImage: "trash") }
                    }
                }
            } header: {
                HStack {
                    Text("Routes")
                    Spacer()
                    sortMenu
                }
            }
        }
        .listStyle(.insetGrouped)
        .navigationTitle("XP Router")
        .searchable(text: $search, prompt: "Routes, games or Pokémon")
        .onChange(of: search) { Task { await reload() } }
        .refreshable { await reload(refresh: true) }
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button { model.path.append(.settings) } label: { Image(systemName: "gearshape") }.accessibilityLabel("Settings")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button { showNew = true } label: { Image(systemName: "plus.circle.fill").font(.title3) }
                    .accessibilityLabel("New Route")
            }
        }
        .task(id: model.revision) { await reload() }
        .task {
            await reload(refresh: true)
            if model.smokeSheet == "new" { model.smokeSheet = nil; showNew = true }
        }
        .sheet(isPresented: $showNew) {
            NewRouteSheet { Task { await model.routeChanged(); model.path = [.editor] } }
        }
        .fileImporter(isPresented: $importing, allowedContentTypes: [.json], allowsMultipleSelection: true) { result in
            guard case .success(let urls) = result else { return }
            Task {
                for url in urls {
                    let access = url.startAccessingSecurityScopedResource()
                    if let r = await model.run("route.import", ["path": .string(url.path)]) {
                        model.show("Imported \(r.name.str)", kind: "saved")
                    }
                    if access { url.stopAccessingSecurityScopedResource() }
                }
                await reload(refresh: true)
            }
        }
        .alert("Rename Route", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("Name", text: $renameText)
            Button("Cancel", role: .cancel) {}
            Button("Rename") {
                let old = renaming ?? ""
                Task {
                    await model.run("route.rename", ["name": .string(old), "new_name": .string(renameText)])
                    await model.refreshState()
                    await reload(refresh: true)
                }
            }
        }
        .confirmationDialog("Delete \(deleting ?? "")?", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }), titleVisibility: .visible) {
            Button("Delete Route", role: .destructive) {
                let n = deleting ?? ""
                Task { await model.run("route.delete", ["name": .string(n)]); await reload(refresh: true) }
            }
        } message: {
            Text("The file is moved to the backups folder (outdated_routes) in Files.")
        }
        .confirmationDialog("Discard unsaved changes?", isPresented: Binding(get: { pendingOpen != nil }, set: { if !$0 { pendingOpen = nil } }), titleVisibility: .visible) {
            Button("Save, then Open") {
                let n = pendingOpen ?? ""
                Task { await model.save(); await model.openRoute(n) }
            }
            Button("Open Without Saving", role: .destructive) {
                let n = pendingOpen ?? ""
                Task { await model.openRoute(n) }
            }
        } message: {
            Text("\(model.routeName.isEmpty ? "The open route" : model.routeName) has changes that aren't saved.")
        }
        .sheet(item: Binding(get: { shareURL.map { ShareItem(url: $0) } }, set: { shareURL = $0?.url })) { item in
            ShareSheet(items: [item.url])
        }
    }

    private var sortMenu: some View {
        Menu {
            Picker("Sort", selection: Binding(get: { sort }, set: { v in Task { await model.run("library.set_prefs", ["sort": .string(v)]); await reload() } })) {
                Label("Most Recent", systemImage: "clock").tag("most_recent")
                Label("Alphabetical", systemImage: "textformat").tag("alphabetical")
                Label("Game", systemImage: "gamecontroller").tag("game")
            }
            if sort == "game" {
                Picker("Game", selection: Binding(get: { lib.game.string ?? "All Games" }, set: { v in Task { await model.run("library.set_prefs", ["game": .string(v)]); await reload() } })) {
                    ForEach(lib.games.strings, id: \.self) { Text($0).tag($0) }
                }
            }
        } label: {
            Label(sort == "game" ? (lib.game.string ?? "Game") : (sort == "alphabetical" ? "A–Z" : "Recent"), systemImage: "arrow.up.arrow.down")
                .font(.caption.weight(.semibold))
                .textCase(nil)
        }
    }

    private func open(_ name: String) {
        if model.isOpen && model.app.unsaved.bool && model.routeName != name {
            pendingOpen = name
        } else if model.isOpen && model.routeName == name {
            model.path = [.editor]
        } else {
            Task { await model.openRoute(name) }
        }
    }

    private func reload(refresh: Bool = false) async {
        if let l = await model.run(refresh ? "library.refresh" : "library.list", ["search": .string(search)], quiet: true) {
            withAnimation(.default) { lib = l }
        }
    }

    private func share(_ name: String) async {
        if let r = await model.run("route.file", ["name": .string(name)]) {
            shareURL = URL(fileURLWithPath: r.path.str)
        }
    }
}

struct ShareItem: Identifiable {
    let url: URL
    var id: String { url.path }
}

struct RouteRow: View {
    let route: JSON
    var isCurrent = false
    var body: some View {
        HStack(spacing: 12) {
            ZStack(alignment: .bottomTrailing) {
                BoxArt(version: route.version.string, size: 50)
                PokemonIcon(species: route.species.string, size: 30)
                    .background(Circle().fill(.regularMaterial))
                    .offset(x: 8, y: 6)
            }
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    Text(route.name.str).font(.body.weight(.semibold)).lineLimit(1)
                    if isCurrent { Chip(text: "Open", color: .accentColor) }
                }
                Text("\(route.version.str) · \(route.species.str)").font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 4)
            if let m = route.mtime.double {
                Text(Date(timeIntervalSince1970: m), format: .relative(presentation: .named, unitsStyle: .abbreviated))
                    .font(.caption2).foregroundStyle(.tertiary).multilineTextAlignment(.trailing).lineLimit(1)
            }
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
    }
}

struct OpenRouteCard: View {
    @Environment(AppModel.self) private var model
    var body: some View {
        Button { model.path = [.editor] } label: {
            HStack(spacing: 14) {
                PokemonIcon(species: model.app.species.string, size: 64)
                    .background(Circle().fill(.white.opacity(0.18)))
                VStack(alignment: .leading, spacing: 4) {
                    Text("CONTINUE").font(.caption2.weight(.heavy)).tracking(1).opacity(0.8)
                    Text(model.routeName.isEmpty ? "Untitled Route" : model.routeName).font(.title3.weight(.bold)).lineLimit(1)
                    HStack(spacing: 6) {
                        Text("\(model.app.version.str) · \(model.app.species.str) · Lv \(model.app.final_level.int ?? 0)")
                        if model.app.unsaved.bool { Text("· Unsaved").fontWeight(.semibold) }
                    }
                    .font(.footnote).opacity(0.9).lineLimit(1)
                }
                Spacer()
                Image(systemName: "chevron.right").font(.headline)
            }
            .foregroundStyle(.white)
            .padding(16)
            .background(
                LinearGradient(colors: [Theme.versionColor(model.app.version.str), Color.accentColor], startPoint: .topLeading, endPoint: .bottomTrailing),
                in: RoundedRectangle(cornerRadius: 22, style: .continuous)
            )
        }
        .buttonStyle(.plain)
    }
}

struct QuickActions: View {
    @Environment(AppModel.self) private var model
    @Binding var showNew: Bool
    @Binding var importing: Bool
    var body: some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 10), count: 4), spacing: 10) {
            tile("New", "plus", .accentColor) { showNew = true }
            tile("Compare", "rectangle.split.2x1", .indigo) { model.path.append(.compare(nil)) }
            tile("Map", "map", .teal) { model.path.append(.map(MapRequest(version: model.isOpen ? nil : "__pick__"))) }
            tile("Import", "square.and.arrow.down", .green) { importing = true }
        }
    }

    private func tile(_ title: String, _ symbol: String, _ color: Color, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: 6) {
                Image(systemName: symbol).font(.title3.weight(.semibold))
                    .frame(width: 44, height: 44)
                    .background(color.opacity(0.15), in: RoundedRectangle(cornerRadius: 13, style: .continuous))
                    .foregroundStyle(color)
                Text(title).font(.caption.weight(.medium)).foregroundStyle(.primary)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 10)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        }
        .buttonStyle(.plain)
    }
}

struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]
    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }
    func updateUIViewController(_ vc: UIActivityViewController, context: Context) {}
}
