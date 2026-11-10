import SwiftUI

enum MapCard: Identifiable, Equatable {
    case object(Int)
    case tile(map: Int, grass: Bool, water: Bool)
    case map(Int)
    var id: String {
        switch self {
        case .object(let i): return "o\(i)"
        case .tile(let m, let g, let w): return "t\(m)\(g)\(w)"
        case .map(let m): return "m\(m)"
        }
    }
}

/// The world map of a game: the open route's (with "add to route"), or
/// any game's from the landing page.
struct MapScreen: View {
    @Environment(AppModel.self) private var model
    let request: MapRequest

    @State private var version: String?
    @State private var games: [JSON] = []
    @State private var info: JSON = .null
    @State private var scope = -1
    @State private var scopeInfo: JSON = .null
    @State private var markers: [MapMarker] = []
    @State private var card: MapCard?
    @State private var selected: Int?
    @State private var night = false
    @State private var showPath = false
    @State private var trip: [(CGPoint, String)] = []
    @State private var tripName: String?
    @State private var anchors: [JSON] = []
    @State private var anchorIndex = 0
    @State private var focusTarget: (CGPoint, CGFloat, Int)?
    @State private var token = 0
    @State private var showList = false
    @State private var search = ""
    @State private var results: JSON = .null
    @State private var kinds: Set<String> = ["trainer", "item", "hidden", "warp", "berry"]
    @State private var downloader = ImageryDownloader()
    @State private var imageryRev = 0

    private var base: JSON { version.map { ["version": .string($0)] } ?? [:] }
    private var browsing: Bool { version != nil }
    private var canAdd: Bool { info.can_add.bool }

    var body: some View {
        ZStack {
            if !info.isNull, let size = scopeSize {
                MapCanvas(
                    key: "\(info.game.str)|\(scope)|\(night)|\(imageryRev)",
                    params: params,
                    worldSize: size,
                    maxLevel: scopeInfo.max_level.int ?? 0,
                    fit: fitRect,
                    overlay: overlay,
                    focusTarget: focusTarget,
                    onTap: { p in Task { await tap(p) } }
                )
                .ignoresSafeArea(edges: .bottom)
            } else if request.version == "__pick__" && version == nil {
                gamePicker
            } else {
                ProgressView()
            }
            VStack {
                if !search.isEmpty { searchResults }
                if let im = info.imagery.object["asset"], !info.imagery.installed.bool { imageryBanner(asset: im.str) }
                Spacer()
                if !anchors.isEmpty { focusBanner }
                if showPath { pathBanner }
            }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .searchable(text: $search, prompt: "Maps, trainers or items")
        .toolbar { toolbar }
        .task { await start() }
        .task(id: search) { await runSearch() }
        .onChange(of: kinds) { Task { await loadObjects() } }
        .onChange(of: showPath) { Task { await loadTrip() } }
        .sheet(item: $card, onDismiss: { selected = nil }) { c in
            MapCardSheet(card: c, base: base, canAdd: canAdd, go: { goTo(map: $0) }, onAdded: { Task { await loadObjects() } })
                .presentationDetents([.medium, .large])
                .presentationBackgroundInteraction(.enabled(upThrough: .medium))
        }
        .sheet(isPresented: $showList) { MapListSheet(maps: info.maps.array) { goTo(map: $0) } }
    }

    private var title: String {
        if scope >= 0 { return info.maps.array.first { $0.id.int == scope }?.name.str ?? "Map" }
        return info.title.string ?? "Map"
    }

    private var params: JSON {
        var p = base.object
        p["scope"] = .number(Double(scope))
        p["night"] = .bool(night)
        p["rev"] = .number(Double(imageryRev))
        return .object(p)
    }

    private var scopeSize: CGSize? {
        let s = scopeInfo.size.ints
        guard s.count == 2, s[0] > 0, s[1] > 0 else { return nil }
        return CGSize(width: s[0], height: s[1])
    }

    private var fitRect: CGRect {
        let f = scopeInfo.fit.ints
        guard f.count == 4 else { return .zero }
        return CGRect(x: f[0], y: f[1], width: f[2] - f[0], height: f[3] - f[1])
    }

    private var overlay: MapOverlayData {
        var o = MapOverlayData(objects: markers, selected: selected)
        let a = anchors[safe: anchorIndex]
        if let a, a.scope.int == scope {
            if a.precision.str == "map", let r = a.rect.ints as [Int]?, r.count == 4 {
                o.focusRect = CGRect(x: r[0], y: r[1], width: r[2] - r[0], height: r[3] - r[1])
            } else if let x = a.x.int, let y = a.y.int {
                o.focus = [CGPoint(x: x, y: y)]
            }
        }
        if showPath { o.trip = trip }
        return o
    }

    // MARK: loading

    private func start() async {
        if request.version == "__pick__" {
            games = (await model.run("map.games", quiet: true))?.array ?? []
            return
        }
        version = request.version
        await loadInfo()
        if let ev = request.focusEvent { await focus(event: ev) }
    }

    private func loadInfo() async {
        guard let i = await model.run("map.info", base) else { return }
        info = i
        scope = -1
        await loadScope()
    }

    private func loadScope() async {
        var p = base.object
        p["scope"] = .number(Double(scope))
        scopeInfo = (await model.run("map.scope_size", .object(p), quiet: true)) ?? .null
        await loadObjects()
        await loadTrip()
    }

    private func loadObjects() async {
        var p = base.object
        p["scope"] = .number(Double(scope))
        p["kinds"] = JSON(Array(kinds))
        let raw = (await model.run("map.objects", .object(p), quiet: true))?.array ?? []
        markers = raw.map { MapMarker(index: $0[0].int ?? 0, kind: $0[1].str, point: CGPoint(x: $0[2].int ?? 0, y: $0[3].int ?? 0), routed: $0[4].bool) }
    }

    private func loadTrip() async {
        guard showPath, !browsing else { trip = []; return }
        let t = (await model.run("map.trip", base, quiet: true)) ?? .null
        tripName = t.folder.string
        trip = t.nodes.array.filter { $0.scope.int == scope }.compactMap { n in
            guard let x = n.x.int, let y = n.y.int else { return nil }
            return (CGPoint(x: x, y: y), n.label.str)
        }
    }

    private func focus(event: Int) async {
        guard let a = await model.run("map.focus", ["id": .number(Double(event))]) else { return }
        anchors = a.array
        anchorIndex = 0
        await goToAnchor()
    }

    private func goToAnchor() async {
        guard let a = anchors[safe: anchorIndex] else { return }
        let s = a.scope.int ?? -1
        if s != scope {
            scope = s
            await loadScope()
        }
        try? await Task.sleep(for: .milliseconds(120))
        if let x = a.x.int, let y = a.y.int {
            token += 1
            focusTarget = (CGPoint(x: x, y: y), a.precision.str == "map" ? 1.2 : 2.5, token)
            selected = a["object"].int
        }
    }

    private func goTo(map id: Int) {
        Task {
            guard let a = await model.run("map.anchor_of_map", base.merging(["map": .number(Double(id))])) else { return }
            card = nil
            let s = a.scope.int ?? -1
            if s != scope { scope = s; await loadScope() }
            try? await Task.sleep(for: .milliseconds(120))
            token += 1
            if s >= 0 {
                focusTarget = nil
                await loadObjects()
            } else if let x = a.x.int, let y = a.y.int {
                focusTarget = (CGPoint(x: x, y: y), 1.2, token)
            }
        }
    }

    private func tap(_ p: CGPoint) async {
        var q = base.object
        q["scope"] = .number(Double(scope)); q["x"] = .number(p.x); q["y"] = .number(p.y); q["kinds"] = JSON(Array(kinds))
        guard let hit = await model.run("map.hit", .object(q), quiet: true), !hit.isNull else { return }
        Haptics.tap()
        if let o = hit["object"].int {
            selected = o
            card = .object(o)
        } else if let m = hit.map.int {
            card = (hit.grass.bool || hit.water.bool) ? .tile(map: m, grass: hit.grass.bool, water: hit.water.bool) : .map(m)
        }
    }

    private func runSearch() async {
        try? await Task.sleep(for: .milliseconds(200))
        guard !search.isEmpty, !info.isNull else { results = .null; return }
        results = (await model.run("map.search", base.merging(["query": .string(search)]), quiet: true)) ?? .null
    }

    // MARK: pieces

    private var gamePicker: some View {
        List(games.map { Item($0, key: "version") }) { g in
            Button {
                version = g["version"].str
                Task { await loadInfo() }
            } label: {
                HStack(spacing: 12) {
                    BoxArt(version: g["version"].str, size: 40)
                    Text(g["version"].str).foregroundStyle(.primary)
                }
            }
        }
        .navigationTitle("Choose a Game")
    }

    private var searchResults: some View {
        List {
            if !results.maps.array.isEmpty {
                Section("Maps") {
                    ForEach(results.maps.array.map { Item($0) }) { m in
                        Button { search = ""; goTo(map: m["id"].int ?? 0) } label: { LabeledContent(m["name"].str, value: m["group"].str) }
                    }
                }
            }
            if !results.trainers.array.isEmpty {
                Section("Trainers") {
                    ForEach(results.trainers.array.map { Item($0, key: "name") }) { t in
                        Button { search = ""; anchors = t["anchors"].array; anchorIndex = 0; Task { await goToAnchor() } } label: { Label(t["name"].str, systemImage: "person.fill") }
                    }
                }
            }
            if !results.items.array.isEmpty {
                Section("Items") {
                    ForEach(results.items.array.map { Item($0, key: "name") }) { t in
                        Button { search = ""; anchors = t["anchors"].array; anchorIndex = 0; Task { await goToAnchor() } } label: {
                            LabeledContent(t["name"].str, value: [(t["balls"].int ?? 0) > 0 ? "\(t["balls"].int ?? 0) balls" : nil, (t["hidden"].int ?? 0) > 0 ? "\(t["hidden"].int ?? 0) hidden" : nil].compactMap { $0 }.joined(separator: " · "))
                        }
                    }
                }
            }
        }
        .listStyle(.insetGrouped)
        .frame(maxHeight: 380)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .padding(.horizontal)
        .shadow(radius: 10)
    }

    private func imageryBanner(asset: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Label("This map shows a sketch until its pictures are downloaded (\(ByteCountFormatter.string(fromByteCount: Int64(info.imagery.bytes.int ?? 0), countStyle: .file))).", systemImage: "photo.on.rectangle.angled")
                .font(.footnote)
            if downloader.running {
                ProgressView(value: downloader.progress)
                Button("Cancel", role: .cancel) { downloader.cancel() }.font(.caption)
            } else {
                if let e = downloader.error { Text(e).font(.caption).foregroundStyle(Theme.failure) }
                Button {
                    Task {
                        guard let file = await downloader.download(asset: asset) else { return }
                        if await model.run("map.install_imagery", base.merging(["path": .string(file.path)])) != nil {
                            await loadInfo()
                            imageryRev += 1
                            model.show("Map pictures installed", kind: "saved")
                        }
                    }
                } label: { Label(downloader.error == nil ? "Download Pictures" : "Retry", systemImage: "arrow.down.circle.fill") }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(14)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .padding(.horizontal)
    }

    private var focusBanner: some View {
        HStack {
            Button { anchorIndex = (anchorIndex - 1 + anchors.count) % anchors.count; Task { await goToAnchor() } } label: { Image(systemName: "chevron.left") }
                .disabled(anchors.count < 2)
            VStack(spacing: 1) {
                Text(anchors[safe: anchorIndex]?.map_name.str ?? "").font(.subheadline.weight(.semibold))
                Text("\(anchorIndex + 1) of \(anchors.count)").font(.caption2).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity)
            Button { anchorIndex = (anchorIndex + 1) % anchors.count; Task { await goToAnchor() } } label: { Image(systemName: "chevron.right") }
                .disabled(anchors.count < 2)
            Button { anchors = [] } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary) }
        }
        .padding(12)
        .background(.regularMaterial, in: Capsule())
        .padding(.horizontal)
        .padding(.bottom, 6)
    }

    private var pathBanner: some View {
        Text(trip.isEmpty ? (tripName == nil ? "Select an event inside a folder to see its trip" : "No map positions in \(tripName ?? "this folder")") : "Trip: \(tripName ?? "")")
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 12).padding(.vertical, 8)
            .background(.regularMaterial, in: Capsule())
            .padding(.bottom, 10)
    }

    @ToolbarContentBuilder private var toolbar: some ToolbarContent {
        ToolbarItemGroup(placement: .topBarTrailing) {
            if scope >= 0 {
                Button { Task { scope = -1; await loadScope() } } label: { Image(systemName: "globe.americas") }
            }
            Button { showList = true } label: { Image(systemName: "list.bullet") }
            Menu {
                Section("Markers") {
                    layer("Trainers", "trainer")
                    layer("Items", "item")
                    layer("Hidden items", "hidden")
                    layer("Berries", "berry")
                    layer("Warps", "warp")
                    layer("Signs", "sign")
                    layer("People", "npc")
                    if info.image_world.bool { layer("Obstacles", "obstacle") }
                }
                if info.night.bool { Toggle("Night", isOn: $night) }
                if !browsing { Toggle("Trip path", isOn: $showPath) }
                if browsing {
                    Button("Change Game") { info = .null; version = nil; Task { games = (await model.run("map.games", quiet: true))?.array ?? [] } }
                }
            } label: { Image(systemName: "square.3.layers.3d") }
        }
    }

    private func layer(_ label: String, _ k: String) -> some View {
        Toggle(label, isOn: Binding(get: { kinds.contains(k) }, set: { if $0 { kinds.insert(k) } else { kinds.remove(k) } }))
    }
}

extension JSON {
    func merging(_ other: JSON) -> JSON {
        var o = object
        for (k, v) in other.object { o[k] = v }
        return .object(o)
    }
}

struct MapListSheet: View {
    let maps: [JSON]
    let go: (Int) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var filter = ""
    var body: some View {
        NavigationStack {
            List {
                ForEach(["Cities", "Routes", "Indoors & Dungeons"], id: \.self) { g in
                    let ms = maps.filter { $0.group.str == g && (filter.isEmpty || $0.name.str.localizedCaseInsensitiveContains(filter)) }
                    if !ms.isEmpty {
                        Section(g) {
                            ForEach(ms.map { Item($0) }) { m in
                                Button(m["name"].str) { dismiss(); go(m["id"].int ?? 0) }.foregroundStyle(.primary)
                            }
                        }
                    }
                }
            }
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle("Maps")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Done") { dismiss() } } }
        }
    }
}

/// What a tap on the map shows: a marker's card, a grass / water tile's
/// encounters, or the map's trainers.
struct MapCardSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let card: MapCard
    let base: JSON
    let canAdd: Bool
    let go: (Int) -> Void
    let onAdded: () -> Void
    @State private var data: JSON = .null
    @State private var condition: String = ""

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    switch card {
                    case .object: objectCard
                    case .tile: encounterCard
                    case .map: mapCard
                    }
                }
                .padding()
            }
            .background(Theme.page)
            .navigationTitle(navTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .task { await load() }
        }
    }

    private var navTitle: String {
        switch card {
        case .object: return data.kind.str.capitalized
        case .tile(_, let g, let w): return w ? "Water" : (g ? "Tall Grass" : "Wild Pokémon")
        case .map: return data.name.str
        }
    }

    private func load() async {
        switch card {
        case .object(let i): data = (await model.run("map.object", base.merging(["object": .number(Double(i))]))) ?? .null
        case .tile(let m, let g, let w): data = (await model.run("map.encounters", base.merging(["map": .number(Double(m)), "terrain": .string(w ? "water" : (g ? "grass" : ""))]))) ?? .null
        case .map(let m): data = (await model.run("map.map_card", base.merging(["map": .number(Double(m))]))) ?? .null
        }
    }

    private func add(_ method: String, _ p: JSON, _ label: String) {
        Task {
            if let r = await model.mutate(method, p), let id = r.id.int {
                model.focusedEvent = id
                model.show("Added \(label)", kind: "saved")
                Haptics.success()
                onAdded()
                await load()
            }
        }
    }

    @ViewBuilder private var objectCard: some View {
        Text(data.map_name.str).font(.caption).foregroundStyle(.secondary)
        switch data.kind.str {
        case "trainer":
            ForEach(data.trainers.array.map { Item($0, key: "name") }) { t in
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        Text(t["name"].str).font(.title3.weight(.bold))
                        Spacer()
                        if t["defeated"].bool { Chip(text: "Already fought", color: Theme.success, symbol: "checkmark") }
                    }
                    if t["unknown"].bool {
                        Text("No router trainer for this one").foregroundStyle(.secondary)
                    } else {
                        Text("\(t["class"].str) · \(t["location"].str)").font(.subheadline).foregroundStyle(.secondary)
                        TrainerTeamStrip(trainer: t.json)
                        ForEach(Array(t["mons"].array.enumerated()), id: \.offset) { _, m in
                            HStack(spacing: 10) {
                                PokemonIcon(species: m.name.string, size: 32)
                                VStack(alignment: .leading) {
                                    Text("\(m.name.str) Lv.\(m.level.int ?? 0)").font(.subheadline.weight(.semibold))
                                    Text(m.moves.strings.joined(separator: " / ")).font(.caption).foregroundStyle(.secondary)
                                }
                            }
                        }
                        if let ev = t["ev_text"].string { Text(ev).font(.caption).foregroundStyle(.secondary) }
                        HStack {
                            if canAdd {
                                Button {
                                    add("map.add_trainer", ["name": t["name"]], t["name"].str)
                                } label: { Label(data.trainers.array.count > 1 ? "Add \(t["name"].str)" : "Add to Route", systemImage: "plus") }
                                .buttonStyle(.borderedProminent)
                                .disabled(t["defeated"].bool)
                            }
                            if let id = t["event_id"].int, canAdd {
                                Button { dismiss(); model.path.append(.event(id)) } label: { Label("Open Event", systemImage: "arrow.right") }.buttonStyle(.bordered)
                            }
                        }
                    }
                }
                .padding()
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            }
        case "item", "hidden", "berry":
            Card {
                HStack {
                    Image(systemName: data.kind.str == "berry" ? "leaf.fill" : "bag.fill").foregroundStyle(.green).font(.title2)
                    VStack(alignment: .leading) {
                        Text(data.item.string.map { (data.count.int ?? 1) > 1 ? "\($0) ×\(data.count.int ?? 1)" : $0 } ?? (data.raw.string ?? "Item")).font(.title3.weight(.bold))
                        if data.kind.str == "hidden" { Text("Hidden — use the Itemfinder").font(.caption).foregroundStyle(.secondary) }
                    }
                }
                if data.fake.bool { Text("Not a bag item (a Pokémon or a decoy)").foregroundStyle(.secondary) }
                if data.acquired.bool { Label("Already picked up in this route", systemImage: "checkmark").foregroundStyle(Theme.success) }
                if canAdd, let item = data.item.string, data.known.bool, !data.fake.bool {
                    Button { add("map.add_item", ["name": .string(item)], item) } label: { Label("Add to Route", systemImage: "plus") }.buttonStyle(.borderedProminent)
                }
            }
        case "warp":
            Card {
                Text(data.dynamic.bool ? "Destination chosen in-game" : "Warp").font(.headline)
                ForEach(data.destinations.array.map { Item($0) }) { d in
                    Button { dismiss(); go(d["id"].int ?? 0) } label: { Label("Go to \(d["name"].str)", systemImage: "arrow.right.circle.fill") }.buttonStyle(.bordered)
                }
            }
        case "sign", "npc":
            Card {
                if let l = data.label.string { Text(l).font(.headline) }
                Text(data.text.string ?? "No text").font(.body)
            }
        case "obstacle":
            Card {
                Text(data.label.string ?? "Obstacle").font(.headline)
                Label("Needs \(data.needs.str)", systemImage: "hammer.fill")
            }
        default:
            ProgressView()
        }
    }

    @ViewBuilder private var encounterCard: some View {
        Text(data.map_name.str).font(.headline)
        let conds = data.conditions.array
        if !conds.isEmpty {
            Picker("Condition", selection: $condition) {
                Text("Normal").tag("")
                ForEach(conds.map { Item($0, key: "0") }) { c in Text(c.json[1].str).tag(c.json[0].str) }
            }
            .pickerStyle(.menu)
        }
        let sections = data.sections.array.filter { conds.isEmpty || ($0.condition.string ?? "") == condition || $0.condition.isNull && condition.isEmpty }
        if data.sections.array.isEmpty {
            Text("No wild Pokémon here").foregroundStyle(.secondary)
        }
        ForEach(Array(sections.enumerated()), id: \.offset) { _, s in
            EncounterSection(section: s, canAdd: canAdd) { sp, lv in add("map.add_wild", ["species": .string(sp), "level": .number(Double(lv))], "\(sp) Lv.\(lv)") }
        }
    }

    @ViewBuilder private var mapCard: some View {
        Text("\(data.trainers.array.count) trainers (\(data.remaining.int ?? 0) not yet fought) · \(data.items.int ?? 0) items").font(.subheadline).foregroundStyle(.secondary)
        if canAdd && (data.remaining.int ?? 0) > 0 {
            Button { add("map.add_trainers", ["folder": data.name, "names": data.to_add], "\(data.remaining.int ?? 0) trainers") } label: {
                Label("Add All Trainers Here", systemImage: "person.3.fill").frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
        }
        ForEach(data.trainers.array.map { Item($0, key: "object") }) { t in
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(t["name"].str).font(.subheadline.weight(.semibold))
                        if t["defeated"].bool { Text("fought").font(.caption).foregroundStyle(Theme.success) }
                    }
                    HStack(spacing: 4) {
                        ForEach(Array(t["party"].array.enumerated()), id: \.offset) { _, p in
                            PokemonIcon(species: p.name.string, size: 26)
                            Text("\(p.level.int ?? 0)").font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                    Text([t["money"].int.map { "¥\($0.grouped)" }, t["exp"].int.map { "\($0.grouped) exp" }, t["ev_text"].string].compactMap { $0 }.joined(separator: " · ")).font(.caption2).foregroundStyle(.secondary)
                }
                Spacer()
                if canAdd && t["variants"].int == 1 {
                    Button { add("map.add_trainer", ["name": t["name"]], t["name"].str) } label: { Image(systemName: "plus.circle.fill").font(.title2) }
                        .disabled(t["defeated"].bool)
                }
            }
            .padding(12)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
        if data.has_encounters.bool, case .map(let m) = card {
            NavigationLink { EncounterListView(map: m, base: base, canAdd: canAdd) } label: { Label("Wild Pokémon", systemImage: "leaf") }
                .buttonStyle(.bordered)
        }
    }
}

struct EncounterSection: View {
    let section: JSON
    let canAdd: Bool
    let add: (String, Int) -> Void
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(section.title.str + (section.rate.int.map { "  ·  rate \($0)" } ?? "")).font(.caption.weight(.bold)).foregroundStyle(.secondary)
            ForEach(Array(section.rows.array.enumerated()), id: \.offset) { _, r in
                HStack(spacing: 10) {
                    PokemonIcon(species: r.species.string, size: 30)
                    Text(r.species.str).font(.subheadline)
                    Spacer()
                    if let ev = r.ev.string { Text(ev).font(.caption2).foregroundStyle(.secondary) }
                    Text("Lv \(r.level.str)").font(.caption.monospacedDigit()).frame(width: 52, alignment: .trailing)
                    Text(r.rate.str).font(.caption.monospacedDigit().weight(.semibold)).frame(width: 44, alignment: .trailing)
                    if canAdd {
                        Button { add(r.species.str, r.min_level.int ?? 2) } label: { Image(systemName: "plus.circle.fill") }.buttonStyle(.borderless)
                    }
                }
            }
        }
        .padding(12)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

struct EncounterListView: View {
    @Environment(AppModel.self) private var model
    let map: Int
    let base: JSON
    let canAdd: Bool
    @State private var data: JSON = .null
    var body: some View {
        ScrollView {
            VStack(spacing: 12) {
                ForEach(Array(data.sections.array.enumerated()), id: \.offset) { _, s in
                    EncounterSection(section: s, canAdd: canAdd) { sp, lv in
                        Task {
                            if await model.mutate("map.add_wild", ["species": .string(sp), "level": .number(Double(lv))]) != nil {
                                model.show("Added \(sp) Lv.\(lv)", kind: "saved")
                            }
                        }
                    }
                }
            }
            .padding()
        }
        .background(Theme.page)
        .navigationTitle("Wild Pokémon")
        .task { data = (await model.run("map.encounters", base.merging(["map": .number(Double(map))]))) ?? .null }
    }
}
