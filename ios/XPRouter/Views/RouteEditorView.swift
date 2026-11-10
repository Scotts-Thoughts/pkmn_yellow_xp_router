import SwiftUI

struct RouteEditorView: View {
    @Environment(AppModel.self) private var model
    @State private var search = ""
    @State private var editMode: EditMode = .inactive
    @State private var selection = Set<String>()
    @State private var showFilters = false
    @State private var addAfter: AddTarget?
    @State private var sheet: EditorSheet?
    @State private var transferIDs: [Int]?
    @State private var confirmDelete: [Int]?
    @State private var confirmClose = false
    @State private var quantityFor: JSON?
    @State private var quantityText = ""
    @State private var folderPrompt: FolderPrompt?
    @State private var folderName = ""
    @State private var filters: JSON = .null
    @State private var jumpTargets: [JSON] = []
    @State private var shareURL: URL?

    var body: some View {
        withDialogs(withSheets(mainList))
    }

    private var mainList: some View {
        ScrollViewReader { proxy in
            listBody
                .onChange(of: model.focusedEvent) { _, id in
                    guard let id else { return }
                    Task {
                        try? await Task.sleep(for: .milliseconds(250))
                        withAnimation { proxy.scrollTo(String(id), anchor: .center) }
                    }
                }
        }
    }

    private var listBody: some View {
        List(selection: $selection) {
            Section {
                StatusStrip(filterCount: activeFilters.count) { showFilters = true }
                    .listRowInsets(EdgeInsets(top: 4, leading: 0, bottom: 4, trailing: 0))
                    .listRowBackground(Color.clear)
                if activeFilters.count > 0 {
                    activeFilterRow
                        .listRowInsets(EdgeInsets(top: 0, leading: 0, bottom: 4, trailing: 0))
                        .listRowBackground(Color.clear)
                }
            }
            Section {
                if model.rows.isEmpty {
                    emptyState
                }
                ForEach(model.rows.map { Item($0) }) { r in
                    rowView(r)
                }
                .onMove(perform: move)
            }
        }
        .listStyle(.plain)
        .environment(\.editMode, $editMode)
        .navigationTitle(Binding(get: { model.routeName.isEmpty ? "Untitled Route" : model.routeName }, set: { name in
            Task { await model.run("route.set_name", ["name": .string(name)]); await model.refreshState() }
        }))
        .navigationBarTitleDisplayMode(.inline)
        .toolbarRole(.editor)
        .searchable(text: $search, placement: .navigationBarDrawer(displayMode: .automatic), prompt: "Search events")
        .onChange(of: search) { Task { await applyFilters() } }
        .toolbar { toolbar }
        .task {
            await model.refreshRows()
            await loadFilters()
            jumpTargets = (await model.run("tree.jump_targets", quiet: true))?.array ?? []
            if model.smokeSheet == "add" { model.smokeSheet = nil; addAfter = AddTarget(after: model.rows.first { $0.kind.str == "group" }?.id.int) }
            if model.smokeSheet == "filters" { model.smokeSheet = nil; showFilters = true }
        }
        .onChange(of: model.revision) { Task { jumpTargets = (await model.run("tree.jump_targets", quiet: true))?.array ?? [] } }
    }

    private func withSheets<V: View>(_ v: V) -> some View {
        v
        .sheet(item: $addAfter) { t in AddEventSheet(after: t.after, before: t.before) }
        .sheet(isPresented: $showFilters) { FilterSheet(filters: $filters, apply: { types in Task { await applyFilters(types: types) } }) }
        .sheet(item: $sheet) { s in
            switch s {
            case .setup: SetupSummarySheet()
            case .dvs: CustomDVSheet()
            case .notes(let text, let url): NotesExportSheet(text: text, url: url)
            }
        }
        .sheet(item: Binding(get: { transferIDs.map { IDList(ids: $0) } }, set: { transferIDs = $0?.ids })) { l in TransferSheet(ids: l.ids) }
        .sheet(item: Binding(get: { shareURL.map { ShareItem(url: $0) } }, set: { shareURL = $0?.url })) { item in ShareSheet(items: [item.url]) }
    }

    private func withDialogs<V: View>(_ v: V) -> some View {
        v
        .confirmationDialog("Delete \(confirmDelete?.count ?? 0) event(s)?", isPresented: Binding(get: { confirmDelete != nil }, set: { if !$0 { confirmDelete = nil } }), titleVisibility: .visible) {
            Button("Delete", role: .destructive) {
                let ids = confirmDelete ?? []
                Task { await model.mutate("edit.delete", ["ids": JSON(ids)]); selection.removeAll(); Haptics.warn() }
            }
        } message: { Text("You can undo this.") }
        .confirmationDialog("Close route?", isPresented: $confirmClose, titleVisibility: .visible) {
            Button("Save, then Close") { Task { await model.save(); await model.closeRoute() } }
            Button("Close Without Saving", role: .destructive) { Task { await model.closeRoute() } }
        } message: { Text("This route has unsaved changes.") }
        .alert("Quantity", isPresented: Binding(get: { quantityFor != nil }, set: { if !$0 { quantityFor = nil } })) {
            TextField("Amount", text: $quantityText).keyboardType(.numberPad)
            Button("Cancel", role: .cancel) {}
            Button("Set") {
                let id = quantityFor?.id.int ?? 0
                Task { await model.mutate("edit.set_quantity", ["id": .number(Double(id)), "quantity": .string(quantityText)]) }
            }
        }
        .alert(folderPrompt?.title ?? "", isPresented: Binding(get: { folderPrompt != nil }, set: { if !$0 { folderPrompt = nil } })) {
            TextField("Folder name", text: $folderName)
            Button("Cancel", role: .cancel) {}
            Button(folderPrompt?.action ?? "OK") {
                guard let p = folderPrompt else { return }
                Task {
                    switch p {
                    case .new(let after): await model.mutate("folder.new", ["name": .string(folderName), "after": JSON(after)])
                    case .rename(let old): await model.mutate("folder.rename", ["old": .string(old), "name": .string(folderName)])
                    }
                }
            }
        }
        }

    @ViewBuilder private func rowView(_ r: Item) -> some View {
        let enabled = r["enabled"].bool
        let id = r["id"].int ?? 0
        let kind = r["kind"].str
        EventRow(row: r.json, flattened: model.flattened)
            .tag(r.id)
            .contentShape(Rectangle())
            .onTapGesture { tap(r.json) }
            .swipeActions(edge: .leading, allowsFullSwipe: true) {
                if kind != "item" && kind != "levelup" {
                    Button { toggleEnabled(id, enabled) } label: {
                        Label(enabled ? "Disable" : "Enable", systemImage: enabled ? "eye.slash" : "eye")
                    }
                    .tint(enabled ? .gray : .green)
                }
            }
            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                if kind == "group" || kind == "folder" {
                    Button(role: .destructive) { askDelete([id], row: r.json) } label: { Label("Delete", systemImage: "trash") }
                    Button { addAfter = AddTarget(after: id) } label: { Label("Add After", systemImage: "plus") }.tint(.accentColor)
                }
            }
            .contextMenu { rowMenu(r.json) }
    }

    private func toggleEnabled(_ id: Int, _ enabled: Bool) {
        Task { await model.mutate("edit.set_enabled", ["ids": [.number(Double(id))], "enabled": .bool(!enabled)]) }
    }

    // MARK: rows

    private var emptyState: some View {
        ContentUnavailableView {
            Label(model.filtering ? "No matching events" : "An empty route", systemImage: model.filtering ? "line.3.horizontal.decrease.circle" : "flag.checkered")
        } description: {
            Text(model.filtering ? "Change the search or filters." : "Add the first event: a trainer, an item, a wild Pokémon…")
        } actions: {
            if !model.filtering { Button("Add Event") { addAfter = AddTarget(after: nil) }.buttonStyle(.borderedProminent) }
        }
        .listRowSeparator(.hidden)
    }

    private func tap(_ row: JSON) {
        guard editMode == .inactive else { return }
        let id = row.id.int ?? 0
        if row.kind.str == "folder" {
            Task {
                await model.run("tree.toggle", ["id": .number(Double(id))])
                withAnimation(.snappy) { }
                await model.refreshRows()
            }
            Haptics.tap()
        } else {
            model.focusedEvent = id
            Task { await model.run("tree.select", ["ids": [.number(Double(id))]]) }
            model.path.append(.event(id))
        }
    }

    @ViewBuilder private func rowMenu(_ row: JSON) -> some View {
        let id = row.id.int ?? 0
        let ids: JSON = [.number(Double(id))]
        let kind = row.kind.str
        if kind != "folder" {
            Button { tap(row) } label: { Label("Open", systemImage: "arrow.up.forward.app") }
        }
        if kind == "group" || kind == "folder" {
            Button { addAfter = AddTarget(after: id) } label: { Label("Add Event After", systemImage: "plus") }
            if kind == "group" { Button { addAfter = AddTarget(before: id) } label: { Label("Add Event Before", systemImage: "plus.square.dashed") } }
            Divider()
            Button { Task { await model.mutate("edit.move_up", ["ids": ids]) } } label: { Label("Move Up", systemImage: "arrow.up") }
            Button { Task { await model.mutate("edit.move_down", ["ids": ids]) } } label: { Label("Move Down", systemImage: "arrow.down") }
            Menu {
                Button { Task { await model.mutate("edit.move_folder_up", ["ids": ids]) } } label: { Label("To Previous Folder", systemImage: "arrow.up.to.line") }
                Button { Task { await model.mutate("edit.move_folder_down", ["ids": ids]) } } label: { Label("To Next Folder", systemImage: "arrow.down.to.line") }
                Button { transferIDs = [id] } label: { Label("Transfer to Folder…", systemImage: "folder") }
            } label: { Label("Move to Folder", systemImage: "folder") }
            Button { Task { await model.mutate("edit.set_enabled", ["ids": ids, "enabled": .bool(!row.enabled.bool)]) } } label: {
                Label(row.enabled.bool ? "Disable" : "Enable", systemImage: row.enabled.bool ? "eye.slash" : "eye")
            }
            if kind == "group" {
                HighlightMenu(ids: [id])
                if row.qty.int != nil {
                    Button { quantityText = String(row.qty.int ?? 1); quantityFor = row } label: { Label("Change Quantity", systemImage: "number") }
                }
                Button { Task { await model.mutate("folder.split", ["id": .number(Double(id))]) } } label: { Label("Split Folder Here", systemImage: "scissors") }
            }
            if kind == "folder" {
                Button { folderName = row.name.str; folderPrompt = .rename(row.name.str) } label: { Label("Rename Folder", systemImage: "pencil") }
                Button { folderName = ""; folderPrompt = .new(after: id) } label: { Label("New Folder After", systemImage: "folder.badge.plus") }
            }
            if model.app.has_map.bool && kind == "group" && ["Fight Trainer", "Acquire Item", "Fight Wild Pkmn"].contains(row.type.str) {
                Button { model.path.append(.map(MapRequest(focusEvent: id))) } label: { Label("Show on Map", systemImage: "map") }
            }
            Divider()
            Button(role: .destructive) { askDelete([id], row: row) } label: { Label("Delete", systemImage: "trash") }
        }
    }

    private func askDelete(_ ids: [Int], row: JSON? = nil) {
        // a lone event or an empty folder goes without asking (undo exists)
        if ids.count == 1, let row, row.kind.str == "group" || (row.kind.str == "folder" && row.child_count.int == 0) {
            Task { await model.mutate("edit.delete", ["ids": JSON(ids)]); Haptics.warn() }
        } else {
            confirmDelete = ids
        }
    }

    private func move(from source: IndexSet, to dest: Int) {
        let rows = model.rows
        let moving = source.compactMap { rows[$0].kind.str == "item" || rows[$0].kind.str == "levelup" ? nil : rows[$0].id.int }
        guard !moving.isEmpty else { return }
        var params: JSON = ["ids": JSON(moving)]
        if dest < rows.count {
            var target = rows[dest]
            if target.kind.str == "item" || target.kind.str == "levelup", let p = target.parent.int, let pr = rows.first(where: { $0.id.int == p }) { target = pr }
            params = ["ids": JSON(moving), "before": JSON(target.id.int), "folder": JSON(target.parent.int)]
        } else if let last = rows.last(where: { $0.depth.int == 0 }) {
            params = ["ids": JSON(moving), "after": JSON(last.id.int), "folder": JSON(last.parent.int)]
        }
        Task { await model.mutate("edit.move_to", params); Haptics.tap() }
    }

    // MARK: filters

    private var activeFilters: [JSON] { filters.filters.array.filter { $0.active.bool } }

    private var activeFilterRow: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                ForEach(activeFilters.map { Item($0, key: "type") }) { f in
                    Button {
                        let types = activeFilters.map { $0.type.str }.filter { $0 != f["type"].str }
                        Task { await applyFilters(types: types) }
                    } label: {
                        Chip(text: f["label"].str, color: .accentColor, filled: true, symbol: f["symbol"].str)
                            .overlay(alignment: .trailing) { EmptyView() }
                    }
                }
                Button("Clear") { Task { await applyFilters(types: []) } }.font(.caption.weight(.semibold))
            }
            .padding(.horizontal, 16)
        }
    }

    private func loadFilters() async {
        if let f = await model.run("tree.filters", quiet: true) { filters = f }
    }

    private func applyFilters(types: [String]? = nil) async {
        let t = types ?? activeFilters.map { $0.type.str }
        await model.run("tree.set_filters", ["types": JSON(t), "search": .string(search)], quiet: true)
        await model.refreshRows()
        await loadFilters()
    }

    // MARK: toolbar

    @ToolbarContentBuilder private var toolbar: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            moreMenu
        }
        ToolbarItemGroup(placement: .bottomBar) {
            if editMode == .active {
                editActions
            } else {
                Button { Task { await model.undo() } } label: { Image(systemName: "arrow.uturn.backward") }.accessibilityLabel("Undo")
                    .disabled(!model.app.can_undo.bool)
                Spacer()
                Button { addAfter = AddTarget(after: model.focusedEvent) } label: {
                    Label("Add Event", systemImage: "plus")
                        .font(.headline)
                        .padding(.horizontal, 18).padding(.vertical, 8)
                        .background(Color.accentColor, in: Capsule())
                        .foregroundStyle(.white)
                }
                Spacer()
                Button { Task { await model.save(); Haptics.success() } } label: {
                    Image(systemName: model.app.unsaved.bool ? "square.and.arrow.down.badge.clock" : "square.and.arrow.down")
                }
                .accessibilityLabel("Save")
            }
        }
    }

    private var selectedIDs: [Int] { model.rows.filter { selection.contains($0.id.str) && $0.kind.str != "item" && $0.kind.str != "levelup" }.compactMap { $0.id.int } }

    @ViewBuilder private var editActions: some View {
        let ids = JSON(selectedIDs)
        let none = selectedIDs.isEmpty
        Button { Task { await model.mutate("edit.move_up", ["ids": ids]) } } label: { Image(systemName: "arrow.up") }.disabled(none)
        Button { Task { await model.mutate("edit.move_down", ["ids": ids]) } } label: { Image(systemName: "arrow.down") }.disabled(none)
        Menu {
            Button { Task { await model.mutate("edit.move_folder_up", ["ids": ids]) } } label: { Label("To Previous Folder", systemImage: "arrow.up.to.line") }
            Button { Task { await model.mutate("edit.move_folder_down", ["ids": ids]) } } label: { Label("To Next Folder", systemImage: "arrow.down.to.line") }
            Button { transferIDs = selectedIDs } label: { Label("Transfer to Folder…", systemImage: "folder") }
        } label: { Image(systemName: "folder") }.disabled(none)
        Menu {
            Button { Task { await model.mutate("edit.set_enabled", ["ids": ids, "enabled": true]) } } label: { Label("Enable", systemImage: "eye") }
            Button { Task { await model.mutate("edit.set_enabled", ["ids": ids, "enabled": false]) } } label: { Label("Disable", systemImage: "eye.slash") }
        } label: { Image(systemName: "eye") }.disabled(none)
        Menu { HighlightMenu(ids: selectedIDs) } label: { Image(systemName: "highlighter") }.disabled(none)
        Button(role: .destructive) { confirmDelete = selectedIDs } label: { Image(systemName: "trash") }.disabled(none)
    }

    private var moreMenu: some View {
        Menu {
            Button { withAnimation { editMode = editMode == .active ? .inactive : .active; selection.removeAll() } } label: {
                Label(editMode == .active ? "Done Selecting" : "Select & Reorder", systemImage: "checkmark.circle")
            }
            Section {
                Button { model.path.append(.runSummary) } label: { Label("Run Summary", systemImage: "tablecells") }
                Button { sheet = .setup } label: { Label("Setup Summary", systemImage: "list.bullet.rectangle") }
                if model.app.has_map.bool { Button { model.path.append(.map(MapRequest())) } label: { Label("Map", systemImage: "map") } }
                Button { model.path.append(.compare(["a": ["current": true]])) } label: { Label("Compare With…", systemImage: "rectangle.split.2x1") }
            }
            if !jumpTargets.isEmpty {
                Menu {
                    ForEach(jumpTargets.map { Item($0, key: "label") }) { t in
                        Button(t["label"].str) {
                            let id = t["id"].int ?? 0
                            Task { await model.run("tree.reveal", ["id": .number(Double(id))]); await model.refreshRows() }
                            model.focusedEvent = id
                            model.path.append(.event(id))
                        }
                    }
                } label: { Label("Jump To", systemImage: "arrow.right.circle") }
            }
            Section {
                Button { folderName = ""; folderPrompt = .new(after: model.focusedEvent) } label: { Label("New Folder", systemImage: "folder.badge.plus") }
                Button { Task { await model.run("tree.expand_all", ["expanded": true]); await model.refreshRows() } } label: { Label("Expand All", systemImage: "rectangle.expand.vertical") }
                Button { Task { await model.run("tree.expand_all", ["expanded": false]); await model.refreshRows() } } label: { Label("Collapse All", systemImage: "rectangle.compress.vertical") }
                Button { Task { await model.mutate("folder.purge") } } label: { Label("Remove Empty Folders", systemImage: "folder.badge.minus") }
            }
            Section {
                Button { sheet = .dvs } label: { Label("Custom DVs / IVs", systemImage: "dial.medium") }
                Button { Task { await exportNotes() } } label: { Label("Export Notes", systemImage: "note.text") }
                Button { Task { await shareRoute() } } label: { Label("Share Route File", systemImage: "square.and.arrow.up") }
                Button { Task { await model.save(); Haptics.success() } } label: { Label("Save", systemImage: "square.and.arrow.down") }
                Button { Task { await model.mutate("route.new_from_current") } } label: { Label("New Route From This One", systemImage: "doc.on.doc") }
            }
            Section {
                Button(role: .destructive) {
                    if model.app.unsaved.bool { confirmClose = true } else { Task { await model.closeRoute() } }
                } label: { Label("Close Route", systemImage: "xmark.circle") }
            }
        } label: {
            Image(systemName: "ellipsis.circle")
        }
        .accessibilityLabel("More")
    }

    private func exportNotes() async {
        if model.routeName.isEmpty { model.alert = "Name the route first (tap the title)."; return }
        if let r = await model.run("route.export_notes") {
            sheet = .notes(r.text.str, URL(fileURLWithPath: r.path.str))
        }
    }

    private func shareRoute() async {
        if let r = await model.run("route.file") { shareURL = URL(fileURLWithPath: r.path.str) }
    }
}

struct AddTarget: Identifiable {
    let id = UUID()
    var after: Int? = nil
    var before: Int? = nil
}

struct IDList: Identifiable {
    let ids: [Int]
    var id: String { ids.map(String.init).joined(separator: ",") }
}

enum EditorSheet: Identifiable {
    case setup, dvs
    case notes(String, URL)
    var id: String {
        switch self {
        case .setup: return "setup"
        case .dvs: return "dvs"
        case .notes: return "notes"
        }
    }
}

enum FolderPrompt {
    case new(after: Int?)
    case rename(String)
    var title: String { if case .rename = self { return "Rename Folder" } else { return "New Folder" } }
    var action: String { if case .rename = self { return "Rename" } else { return "Create" } }
}

struct HighlightMenu: View {
    @Environment(AppModel.self) private var model
    let ids: [Int]
    var body: some View {
        Menu {
            Button { Task { await model.mutate("edit.highlight", ["ids": JSON(ids), "n": .null]) } } label: { Label("Clear Highlight", systemImage: "xmark") }
            ForEach(1...9, id: \.self) { n in
                Button { Task { await model.mutate("edit.highlight", ["ids": JSON(ids), "n": .number(Double(n))]) } } label: {
                    Label("Highlight \(n)", systemImage: "\(n).circle.fill")
                }
            }
        } label: { Label("Highlight", systemImage: "highlighter") }
    }
}


/// The version / validity / level strip at the top of the editor.
struct StatusStrip: View {
    @Environment(AppModel.self) private var model
    var filterCount = 0
    var onFilter: () -> Void = {}
    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                PokemonIcon(species: model.app.species.string, size: 30)
                Chip(text: model.app.version.str, color: Theme.versionColor(model.app.version.str), filled: true)
                Chip(text: "Lv \(model.app.final_level.int ?? 0) at end", color: .secondary)
                Button {
                    Task {
                        if let id = (await model.run("tree.next_invalid", quiet: true))?.int {
                            await model.refreshRows()
                            model.focusedEvent = id
                            model.path.append(.event(id))
                        }
                    }
                } label: {
                    Chip(text: model.app.valid.bool ? "Valid" : "\(model.app.invalid_ids.array.count) invalid", color: model.app.valid.bool ? Theme.success : Theme.failure, filled: !model.app.valid.bool, symbol: model.app.valid.bool ? "checkmark.seal.fill" : "exclamationmark.triangle.fill")
                }
                .disabled(model.app.valid.bool)
                if model.app.unsaved.bool { Chip(text: "Unsaved", color: Theme.warning, symbol: "circle.fill") }
                Button(action: onFilter) {
                    Chip(text: filterCount == 0 ? "Filter" : "\(filterCount) filter\(filterCount == 1 ? "" : "s")", color: .accentColor, filled: filterCount > 0, symbol: "line.3.horizontal.decrease")
                }
                .accessibilityLabel("Filter")
            }
            .padding(.horizontal, 16)
        }
    }
}

struct EventRow: View {
    let row: JSON
    var flattened = false
    private var kind: String { row.kind.str }
    private var tagColor: Color? { row.color.string.map { Color(hex: $0) } }

    var body: some View {
        HStack(spacing: 10) {
            if kind == "folder" {
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(.secondary)
                    .rotationEffect(.degrees(row.expanded.bool ? 90 : 0))
                    .frame(width: 14)
                    .animation(.snappy, value: row.expanded.bool)
            }
            icon
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(title)
                        .font(kind == "folder" ? .body.weight(.semibold) : (kind == "levelup" ? .subheadline : .body))
                        .foregroundStyle(row.enabled.bool ? (kind == "levelup" ? Color.secondary : (row.faded.bool ? Color.secondary : Color.primary)) : Color.secondary)
                        .strikethrough(!row.enabled.bool, color: .secondary)
                        .lineLimit(2)
                    if row.notes.bool { Image(systemName: "note.text").font(.caption2).foregroundStyle(.tertiary) }
                }
                if let sub = subtitle { Text(sub).font(.caption).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer(minLength: 4)
            trailing
        }
        .padding(.vertical, kind == "levelup" ? 1 : 3)
        .padding(.leading, CGFloat(flattened ? 0 : (row.depth.int ?? 0)) * 16)
        .opacity(row.enabled.bool ? 1 : 0.55)
        .listRowBackground(rowBackground)
    }

    private var title: String {
        var n = row.name.str
        if kind == "group", let q = row.qty.int, q > 1, !n.contains("x\(q)") { n += " ×\(q)" }
        return n
    }

    private var subtitle: String? {
        guard kind == "group", row.enabled.bool, let lvl = row.level.int else {
            if kind == "folder" { return "\(row.child_count.int ?? 0) events" }
            return nil
        }
        var parts: [String] = ["Lv \(lvl)"]
        if let g = row.xp_gain.int, g > 0 { parts.append("+\(g.grouped) xp") }
        if let pct = row.pct_tnl.string, !pct.isEmpty { parts.append("\(pct)\(pct.hasSuffix("%") ? "" : "%") to next") }
        if let eps = row.eps.string, !eps.isEmpty { parts.append("\(eps) xp/s") }
        let lu = row.levels_up.str
        if !lu.isEmpty { parts.append("↑ \(lu)") }
        return parts.joined(separator: " · ")
    }

    @ViewBuilder private var icon: some View {
        if kind == "group", let sp = row.species.string {
            PokemonIcon(species: sp, size: 34)
                .overlay(alignment: .bottomTrailing) { badge }
        } else {
            Image(systemName: row.symbol.str.isEmpty ? "circle.fill" : row.symbol.str)
                .font(.system(size: kind == "levelup" ? 11 : 14, weight: .semibold))
                .foregroundStyle(kind == "folder" ? Color.accentColor : .white)
                .frame(width: kind == "levelup" ? 24 : 34, height: kind == "levelup" ? 24 : 34)
                .background(kind == "folder" ? Color.accentColor.opacity(0.14) : iconColor, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(alignment: .bottomTrailing) { badge }
        }
    }

    @ViewBuilder private var badge: some View {
        if row.errors.bool {
            Image(systemName: "exclamationmark.circle.fill").font(.caption).foregroundStyle(.white, Theme.failure).offset(x: 4, y: 4)
        } else if row.warnings.bool {
            Image(systemName: "exclamationmark.triangle.fill").font(.caption2).foregroundStyle(Theme.warning).offset(x: 4, y: 4)
        }
    }

    private var iconColor: Color {
        switch row.type.str {
        case "Fight Trainer": return Color(hex: "#ff6b3d")
        case "Fight Wild Pkmn": return Color(hex: "#34a853")
        case "Use Rare Candy": return Color(hex: "#c47dff")
        case "Use Vitamin": return Color(hex: "#ff4f8b")
        case "Learn Levelup Move", "Learn TM/HM Move": return Color(hex: "#4a8cff")
        case "Acquire Item", "Purchase Item", "Sell Item", "Use/Drop Item", "Hold Item", "Reorder Bag": return Color(hex: "#f5a623")
        case "PkmnCenter Heal", "Game Save", "Blackout": return Color(hex: "#5ac8fa")
        case "Evolution": return Color(hex: "#af52de")
        case "Just Notes": return Color(hex: "#8e8e93")
        default: return Color(hex: "#64748b")
        }
    }

    @ViewBuilder private var trailing: some View {
        if kind == "group", row.enabled.bool, let lvl = row.level.int {
            VStack(alignment: .trailing, spacing: 1) {
                Text("\(lvl)").font(.system(.subheadline, design: .rounded).weight(.bold)).monospacedDigit()
                if let lg = row.level_gain.string, !lg.isEmpty, lg != "0" { Text("+\(lg)").font(.caption2.weight(.semibold)).foregroundStyle(Theme.success) }
            }
        } else if kind == "folder", let c = tagColor {
            Circle().fill(c).frame(width: 8, height: 8)
        }
    }

    @ViewBuilder private var rowBackground: some View {
        if let c = tagColor, row.tag.str != "important" {
            HStack(spacing: 0) {
                Rectangle().fill(c).frame(width: 4)
                Rectangle().fill(c.opacity(row.errors.bool ? 0.16 : 0.10))
            }
        } else if row.major.bool {
            Color.primary.opacity(0.035)
        } else {
            Color(uiColor: .systemBackground)
        }
    }
}
