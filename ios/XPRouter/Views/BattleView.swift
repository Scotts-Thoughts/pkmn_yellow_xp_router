import SwiftUI

/// The battle summary: damage ranges and kill chances for every matchup,
/// with the fight's setup controls (pre-fight candies, vitamins, held item,
/// setup moves, weather, screens, Intimidate, Mimic / Metronome / multi-hit
/// choices, stat stages, test moves).
struct BattleView: View {
    @Environment(AppModel.self) private var model
    let eventID: Int
    var onAssign: (Int) -> Void
    @State private var view: JSON = .null
    @State private var loading = true
    @State private var showConfig = false
    @State private var heldItems: [String] = []
    @State private var exportItem: ExportImage?
    @State private var lastCandy = Date.distantPast

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 14) {
                if loading && view.isNull {
                    ProgressView().padding(.top, 60)
                } else if !view.loaded.bool {
                    ContentUnavailableView("No battle", systemImage: "figure.fencing", description: Text("This event has no Pokémon to fight."))
                } else {
                    if view.can_edit.bool { controls }
                    if view.show_legacy.bool { LegacyControls(view: view) { await act($0) } }
                    ForEach(view.matchups.array.map { Item($0, key: "idx") }) { m in
                        MatchupCard(m: m.json, view: view, count: view.matchups.array.count, act: { await act($0) }, onAssign: onAssign, onExport: { mode in export(m.json, mode) })
                    }
                    footer
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
        }
        .background(Theme.page)
        .task(id: eventID) { await load() }
        .onChange(of: model.revision) { Task { await refresh() } }
        .sheet(isPresented: $showConfig, onDismiss: { Task { await act(["op": "refresh"]) } }) { BattleConfigSheet() }
        .sheet(item: $exportItem) { e in ShareSheet(items: [e.image]) }
    }

    // MARK: controls

    private var controls: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Chip(text: "HP \(view.hp.int ?? 0)", color: .red, symbol: "heart.fill")
                Chip(text: "Spe \(view.speed.int ?? 0)", color: .pink, symbol: "hare.fill")
                if view["double"].bool { Chip(text: "Double battle", color: .orange) }
                Spacer()
                Button { showConfig = true } label: { Image(systemName: "slider.horizontal.3") }
            }
            HStack {
                Image(systemName: "birthday.cake.fill").foregroundStyle(.purple)
                Text("Pre-fight Rare Candies")
                Spacer()
                CountStepper(value: view.candies.int ?? 0) { delta in
                    let now = Date()
                    let coalesce = now.timeIntervalSince(lastCandy) < 1.0
                    lastCandy = now
                    Task { await act(["op": "candies", "value": .number(Double(max(0, (view.candies.int ?? 0) + delta))), "coalesce": .bool(coalesce)]) }
                }
            }
            if !view.vitamins.array.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Vitamins used before this fight").font(.caption).foregroundStyle(.secondary)
                    LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 8) {
                        ForEach(view.vitamins.array.map { Item($0, key: "stat") }) { v in
                            VStack(spacing: 4) {
                                Text(v["label"].str).font(.caption2.weight(.bold)).foregroundStyle(.secondary)
                                CountStepper(value: v["count"].int ?? 0, compact: true) { d in
                                    Task { await act(["op": "vitamin", "stat": v["stat"], "delta": .number(Double(d))]) }
                                }
                            }
                            .padding(6)
                            .frame(maxWidth: .infinity)
                            .background(Theme.well, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        }
                    }
                }
            }
            if view.has_held_items.bool {
                PickerRow(label: "Held item", value: view.held_item.str, options: heldItems, placeholder: "None") { v in
                    Task { await act(["op": "held_item", "value": .string(v)]) }
                }
                .task { heldItems = (await model.run("battle.held_items", quiet: true))?.strings ?? [] }
            }
        }
        .padding(14)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
    }

    private var footer: some View {
        HStack(spacing: 6) {
            Image(systemName: "info.circle")
            Text("Tap one of your moves to highlight it; long-press for more.")
        }
        .font(.caption).foregroundStyle(.secondary)
        .padding(.top, 4)
    }

    // MARK: data

    private func load() async {
        loading = true
        if let v = await model.run("battle.load", ["id": .number(Double(eventID))], quiet: true) { view = v }
        loading = false
    }

    private func refresh() async {
        if let v = await model.run("battle.view", quiet: true), v.event_id.int == eventID { view = v } else { await load() }
    }

    private func act(_ params: JSON) async {
        if let v = await model.run("battle.action", params) {
            withAnimation(.snappy(duration: 0.2)) { view = v }
            Haptics.tap()
            await model.refreshState()
            await model.refreshRows()
        }
    }

    @MainActor private func export(_ m: JSON, _ mode: String) {
        let card = MatchupCard(m: m, view: view, count: 1, act: { _ in }, onAssign: { _ in }, onExport: { _ in }, exportMode: mode)
            .frame(width: 400)
            .padding(10)
            .background(Theme.page)
            .environment(model)
        let r = ImageRenderer(content: card)
        r.scale = 3
        if let img = r.uiImage { exportItem = ExportImage(image: img) }
    }
}

struct ExportImage: Identifiable {
    let id = UUID()
    let image: UIImage
}

struct CountStepper: View {
    let value: Int
    var compact = false
    let change: (Int) -> Void
    var body: some View {
        HStack(spacing: compact ? 6 : 10) {
            Button { change(-1) } label: { Image(systemName: "minus").frame(width: compact ? 22 : 30, height: compact ? 22 : 30) }
                .disabled(value <= 0)
            Text("\(value)").font((compact ? Font.subheadline : .headline).monospacedDigit()).frame(minWidth: 22)
            Button { change(1) } label: { Image(systemName: "plus").frame(width: compact ? 22 : 30, height: compact ? 22 : 30) }
        }
        .buttonStyle(.bordered)
        .buttonBorderShape(.circle)
        .controlSize(.small)
    }
}

struct MatchupCard: View {
    let m: JSON
    let view: JSON
    let count: Int
    let act: (JSON) async -> Void
    let onAssign: (Int) -> Void
    let onExport: (String) -> Void
    /// set when rendering an image: "full", "player" or "enemy"
    var exportMode: String? = nil

    private var idx: Int { m.idx.int ?? 0 }
    private var collapsed: Bool { exportMode == nil && m.collapsed.bool }
    private var speedColor: Color {
        switch m.speed_cmp.int {
        case 1: return Color(hex: "#3498db")
        case 0: return Color(hex: "#e6b800")
        case -1: return Color(hex: "#e74c3c")
        default: return .primary
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            if !collapsed {
                if exportMode != "enemy" {
                    side(title: "\(m.player.name.str) Lv\(m.player.level.int ?? 0) damage", moves: m.player_moves.array, player: true)
                }
                if exportMode != "player" {
                    if view.test_moves_enabled.bool {
                        side(title: "Test moves", moves: m.test_moves.array, player: true, test: true)
                    } else {
                        side(title: "\(m.enemy.name.str) Lv\(m.enemy.level.int ?? 0) damage", moves: m.enemy_moves.array, player: false)
                    }
                }
            }
        }
        .padding(14)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
    }

    private var header: some View {
        HStack(spacing: 10) {
            ZStack(alignment: .bottomLeading) {
                PokemonIcon(species: m.enemy.name.string, size: 48)
                PokemonIcon(species: m.player.name.string, size: 26)
                    .background(Circle().fill(.regularMaterial))
                    .offset(x: -6, y: 6)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text("\(m.enemy.name.str) Lv\(m.enemy.level.int ?? 0)").font(.headline)
                HStack(spacing: 6) {
                    Text(speedText).font(.caption.weight(.semibold)).foregroundStyle(speedColor)
                    if let hp = m.enemy.hp.int { Text("· \(hp) HP").font(.caption).foregroundStyle(.secondary) }
                    if let d = m.download.string { Text("· Download: \(d == "attack" ? "Atk" : "Sp. Atk") +1").font(.caption).foregroundStyle(.secondary) }
                }
                if let types = m.enemy_detail.types.array.first, !types.isNull {
                    HStack(spacing: 3) { ForEach(m.enemy_detail.types.strings, id: \.self) { TypeBadge(type: $0) } }
                }
            }
            Spacer()
            if exportMode == nil {
                Menu {
                    Section("Export image") {
                        Button("Matchup") { onExport("full") }
                        Button("Player ranges") { onExport("player") }
                        Button("Enemy ranges") { onExport("enemy") }
                    }
                    if view.can_edit.bool && count > 1 {
                        Section("Battle order") {
                            if idx > 0 { Button { Task { await act(["op": "reorder", "from": .number(Double(idx)), "to": .number(Double(idx - 1))]) } } label: { Label("Move Earlier", systemImage: "arrow.up") } }
                            if idx < count - 1 { Button { Task { await act(["op": "reorder", "from": .number(Double(idx)), "to": .number(Double(idx + 1))]) } } label: { Label("Move Later", systemImage: "arrow.down") } }
                        }
                    }
                } label: { Image(systemName: "ellipsis.circle").font(.title3) }
                Button {
                    Task { await act(["op": "collapse", "mon": .number(Double(idx)), "collapsed": .bool(!m.collapsed.bool)]) }
                } label: {
                    Image(systemName: "chevron.down").rotationEffect(.degrees(m.collapsed.bool ? -90 : 0)).font(.headline)
                }
            }
        }
        .overlay(alignment: .bottomTrailing) { intimidates.offset(y: 22) }
        .padding(.bottom, hasIntimidate ? 18 : 0)
    }

    private var hasIntimidate: Bool { !m.player_intimidate.isNull || !m.enemy_intimidate.isNull }

    @ViewBuilder private var intimidates: some View {
        HStack(spacing: 8) {
            ForEach([("player_intimidate", true), ("enemy_intimidate", false)], id: \.0) { key, player in
                let it = m[key]
                if !it.isNull && (exportMode == nil || it.active.bool) {
                    Toggle(isOn: Binding(get: { it.active.bool }, set: { v in Task { await act(["op": "intimidate", "mon": .number(Double(idx)), "player": .bool(player), "enabled": .bool(v)]) } })) {
                        Text("\(player ? "Your" : "Foe's") Intimidate").font(.caption2.weight(.semibold))
                    }
                    .toggleStyle(.button)
                    .controlSize(.mini)
                    .disabled(it.blocked.bool || exportMode != nil)
                }
            }
        }
    }

    /// every slot for test moves; otherwise only the moves the mon has
    private func shownSlots(_ moves: [JSON], test: Bool) -> [Int] {
        if test { return Array(0..<4) }
        let s = (0..<moves.count).filter { !moves[$0].isNull }
        return s.isEmpty ? [0] : s
    }

    private var speedText: String {
        switch m.speed_cmp.int {
        case 1: return "You outspeed (\(m.player.speed.int ?? 0) vs \(m.enemy.speed.int ?? 0))"
        case 0: return "Speed tie (\(m.player.speed.int ?? 0))"
        case -1: return "Outsped (\(m.player.speed.int ?? 0) vs \(m.enemy.speed.int ?? 0))"
        default: return ""
        }
    }

    private func side(title: String, moves: [JSON], player: Bool, test: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title.uppercased()).font(.caption2.weight(.bold)).foregroundStyle(.secondary).tracking(0.5)
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)], spacing: 8) {
                ForEach(shownSlots(moves, test: test), id: \.self) { k in
                    let mv = moves[safe: k] ?? .null
                    if test && idx == 0 {
                        TestMoveSlot(slot: k, current: view.test_moves[k].str, move: mv, act: act)
                    } else if mv.isNull {
                        RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Theme.well.opacity(0.5)).frame(minHeight: 60)
                            .overlay(Text("—").foregroundStyle(.tertiary))
                    } else {
                        MoveTile(mv: mv, mon: idx, slot: k, player: player, exporting: exportMode != nil, act: act, onAssign: onAssign)
                    }
                }
            }
        }
    }
}

struct MoveTile: View {
    let mv: JSON
    let mon: Int
    let slot: Int
    let player: Bool
    var exporting = false
    let act: (JSON) async -> Void
    let onAssign: (Int) -> Void

    private var hl: Color? { Theme.highlight(mv.highlight.int ?? 0) }
    private var best: Bool { mv.best.bool }
    private var faded: Bool { mv.faded.bool }
    private var flag: Color { player ? Theme.success : Theme.failure }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            nameBar
            VStack(alignment: .leading, spacing: 4) {
                if mv.damaging.bool {
                    HStack {
                        Text(mv.range.str).fontWeight(.semibold)
                        Spacer()
                        Text(mv.range_pct.str).foregroundStyle(.secondary)
                    }
                    .font(.footnote.monospacedDigit())
                    HStack {
                        Label(mv.crit.str, systemImage: "bolt.fill").labelStyle(CritLabel())
                        Spacer()
                        Text(mv.crit_pct.str)
                    }
                    .font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                    if !mv.kills.array.isEmpty || !mv.recoil.isNull {
                        Divider().padding(.vertical, 2)
                    }
                    ForEach(Array(mv.kills.array.enumerated()), id: \.offset) { _, k in
                        HStack {
                            Text(k.text.str)
                            Spacer()
                            Text(k.pct.str).monospacedDigit()
                        }
                        .font(.caption.weight(best ? .bold : .regular))
                        .foregroundStyle(best ? flag : .primary)
                    }
                    if !mv.recoil.isNull {
                        HStack { Text(mv.recoil.text.str); Spacer(); Text(mv.recoil.pct.str) }
                            .font(.caption2).foregroundStyle(Theme.failure)
                    }
                } else {
                    Text("Status move").font(.caption).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading)
                }
                controls
            }
            .padding(8)
        }
        .frame(maxWidth: .infinity, minHeight: 60, alignment: .topLeading)
        .background(best ? flag.opacity(0.12) : Theme.well, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(best ? flag.opacity(0.55) : .clear, lineWidth: 1.5))
        .opacity(faded ? 0.45 : 1)
        .contextMenu {
            if player && !mv.test.bool {
                Button { Task { await act(["op": "highlight", "mon": .number(Double(mon)), "move": .number(Double(slot))]) } } label: { Label("Cycle Highlight", systemImage: "highlighter") }
                Button { Task { await act(["op": "highlight", "mon": .number(Double(mon)), "move": .number(Double(slot)), "reset": true]) } } label: { Label("Clear Highlight", systemImage: "xmark") }
                Button { onAssign(slot) } label: { Label("Assign Another Move to Slot \(slot + 1)…", systemImage: "arrow.triangle.2.circlepath") }
            }
        }
    }

    private var nameBar: some View {
        HStack(spacing: 6) {
            Text(mv.name.str).font(.subheadline.weight(.bold)).lineLimit(1).minimumScaleFactor(0.7)
            Spacer(minLength: 2)
            if let w = mv.weather.object["name"]?.string, !exporting || mv.weather.active.bool {
                Button { Task { await act(["op": "weather_move", "mon": .number(Double(mon)), "name": mv.weather.move, "enabled": .bool(!mv.weather.active.bool)]) } } label: {
                    Image(systemName: weatherSymbol(w)).symbolVariant(mv.weather.active.bool ? .fill : .none)
                }
                .buttonStyle(.plain)
                .foregroundStyle(mv.weather.active.bool ? (hl == nil ? Color.accentColor : .white) : .secondary)
            }
            if !mv.screen.isNull, !exporting || mv.screen.active.bool {
                Button { Task { await act(["op": "screen_move", "mon": .number(Double(mon)), "player": .bool(player), "name": mv.screen.move, "enabled": .bool(!mv.screen.active.bool)]) } } label: {
                    Image(systemName: "shield.lefthalf.filled").symbolVariant(mv.screen.active.bool ? .fill : .none)
                }
                .buttonStyle(.plain)
                .foregroundStyle(mv.screen.active.bool ? (hl == nil ? Color.accentColor : .white) : .secondary)
            }
        }
        .padding(.horizontal, 8).padding(.vertical, 6)
        .foregroundStyle(hl == nil ? Color.primary : .white)
        .background(alignment: .leading) {
            ZStack(alignment: .leading) {
                (hl ?? Theme.typeColor(mv.type.string).opacity(0.16))
                if hl == nil { Rectangle().fill(Theme.typeColor(mv.type.string)).frame(width: 4) }
            }
        }
        .clipShape(UnevenRoundedRectangle(topLeadingRadius: 14, topTrailingRadius: 14))
        .contentShape(Rectangle())
        .onTapGesture {
            guard player, !mv.test.bool, !exporting else { return }
            Task { await act(["op": "highlight", "mon": .number(Double(mon)), "move": .number(Double(slot))]) }
        }
    }

    @ViewBuilder private var controls: some View {
        let custom = mv.custom
        let stage = mv.stage
        if !custom.isNull || !stage.isNull {
            HStack(spacing: 6) {
                if !custom.isNull {
                    let sel = custom.selection.str
                    if !exporting || (custom.kind.str == "metronome" ? !sel.isEmpty : sel != custom.options.array.first?.str) {
                        Menu {
                            ForEach(custom.options.strings, id: \.self) { o in
                                Button(o.isEmpty ? "No pick" : o) {
                                    Task {
                                        if custom.kind.str == "mimic" {
                                            await act(["op": "mimic", "value": .string(o)])
                                        } else {
                                            await act(["op": "custom_data", "mon": .number(Double(mon)), "move": .number(Double(slot)), "player": .bool(player), "value": .string(o)])
                                        }
                                    }
                                }
                            }
                        } label: {
                            Text(sel.isEmpty ? (custom.kind.str == "metronome" ? "Pick move" : "Choose") : sel)
                                .font(.caption2.weight(.semibold)).lineLimit(1)
                                .padding(.horizontal, 7).padding(.vertical, 4)
                                .background(Color.accentColor.opacity(0.14), in: Capsule())
                        }
                        .disabled(exporting)
                    }
                }
                if !stage.isNull, !exporting || stage.selection.str != "0" {
                    let opts = stage.options.strings
                    let cur = stage.selection.str
                    let i = opts.firstIndex(of: cur) ?? 0
                    HStack(spacing: 4) {
                        Button { if i > 0 { send(opts[i - 1]) } } label: { Image(systemName: "minus.circle.fill") }.disabled(i == 0 || exporting)
                        Text("×\(cur)").font(.caption2.weight(.bold).monospacedDigit())
                        Button { if i < opts.count - 1 { send(opts[i + 1]) } } label: { Image(systemName: "plus.circle.fill") }.disabled(i >= opts.count - 1 || exporting)
                    }
                    .font(.caption)
                    .buttonStyle(.plain)
                    .foregroundStyle(.tint)
                    .help("Times this stat move is used")
                }
            }
            .padding(.top, 2)
        }
    }

    private func send(_ v: String) {
        Task { await act(["op": "stat_stage", "mon": .number(Double(mon)), "move": .number(Double(slot)), "player": .bool(player), "value": .string(v)]) }
    }

    private func weatherSymbol(_ w: String) -> String {
        switch w {
        case "Rain": return "cloud.rain"
        case "Harsh Sunlight": return "sun.max"
        case "Sandstorm": return "wind"
        case "Hail": return "cloud.hail"
        case "Fog": return "cloud.fog"
        default: return "cloud"
        }
    }
}

struct CritLabel: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 3) { configuration.icon.font(.system(size: 8)); configuration.title }
    }
}

/// Matchup 1's test-move slots pick the move.
struct TestMoveSlot: View {
    @Environment(AppModel.self) private var model
    let slot: Int
    let current: String
    let move: JSON
    let act: (JSON) async -> Void
    @State private var picking = false
    @State private var moves: [String] = []
    var body: some View {
        VStack(spacing: 4) {
            Button { picking = true } label: {
                HStack {
                    Text(current.isEmpty ? "Test move \(slot + 1)" : current).font(.subheadline.weight(.semibold)).lineLimit(1)
                    Spacer()
                    Image(systemName: "chevron.up.chevron.down").font(.caption2)
                }
                .padding(8)
                .background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            }
            .buttonStyle(.plain)
            if !move.isNull { MoveTile(mv: move, mon: 0, slot: slot + 4, player: true, act: act, onAssign: { _ in }) }
        }
        .sheet(isPresented: $picking) {
            ListPickerSheet(title: "Test Move \(slot + 1)", options: [""] + moves, current: current) { v in
                Task { await act(["op": "test_move", "slot": .number(Double(slot)), "value": .string(v)]) }
            }
            .task { moves = (await model.run("opts.moves", quiet: true))?.strings ?? [] }
        }
    }
}

/// Player / enemy setup moves, transform and weather (the desktop's
/// "Legacy Controls").
struct LegacyControls: View {
    let view: JSON
    let act: (JSON) async -> Void
    @State private var open = false
    var body: some View {
        DisclosureGroup(isExpanded: $open) {
            VStack(alignment: .leading, spacing: 12) {
                setupRow("Player setup", key: "player_setup", op: "player_setup")
                Toggle("Transformed (Transform / Imposter)", isOn: Binding(get: { view.transformed.bool }, set: { v in Task { await act(["op": "transform", "enabled": .bool(v)]) } }))
                setupRow("Enemy setup", key: "enemy_setup", op: "enemy_setup")
                Picker("Weather", selection: Binding(get: { view.weather.str }, set: { v in Task { await act(["op": "weather", "value": .string(v)]) } })) {
                    ForEach(view.weather_options.strings, id: \.self) { Text($0).tag($0) }
                }
                Text(view["double"].bool ? "Double battle" : "Single battle").font(.caption).foregroundStyle(.secondary)
            }
            .padding(.top, 8)
        } label: {
            Label("Setup, weather & transform", systemImage: "wand.and.stars").font(.subheadline.weight(.semibold))
        }
        .padding(14)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
    }

    private func setupRow(_ title: String, key: String, op: String) -> some View {
        let list = view[key].strings
        return VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title).font(.subheadline)
                Spacer()
                Menu {
                    ForEach(view.setup_options.strings, id: \.self) { m in
                        Button(m) { Task { await act(["op": .string(op), "moves": JSON(list + [m])]) } }
                    }
                } label: { Image(systemName: "plus.circle.fill") }
                if !list.isEmpty {
                    Button("Reset") { Task { await act(["op": .string(op), "moves": JSON([String]())]) } }.font(.caption)
                }
            }
            Text(list.isEmpty ? "None" : list.joined(separator: ", ")).font(.caption).foregroundStyle(.secondary)
        }
    }
}
