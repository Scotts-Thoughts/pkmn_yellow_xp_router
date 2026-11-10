import SwiftUI

typealias Draft = [String: JSON]

extension Dictionary where Key == String, Value == JSON {
    func s(_ k: String) -> String { self[k]?.string ?? "" }
    func i(_ k: String, _ d: Int = 0) -> Int { self[k]?.int ?? d }
    func b(_ k: String) -> Bool { self[k]?.bool ?? false }
}

/// The fields of one event kind, shared by "Add Event" (create) and the
/// event screen (edit). `context` is the route state at the insertion
/// point (moves, bag, forms) from the core.
struct EventForm: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    let context: JSON
    var editing = false
    var after: Int? = nil
    var before: Int? = nil
    /// the trainer's team (edit mode), from event.detail
    var team: [JSON] = []

    var kind: String { draft.s("kind") }

    var body: some View {
        switch kind {
        case "trainer": TrainerFields(draft: $draft, editing: editing, team: team)
        case "wild": WildFields(draft: $draft)
        case "rare_candy":
            Section { AmountStepper(label: "Rare Candies", value: intBinding("amount", 1), range: 1...999) }
        case "vitamin": VitaminFields(draft: $draft)
        case "get_item", "buy_item", "sell_item", "use_item", "hold_item": ItemFields(draft: $draft, context: context, after: after, before: before)
        case "tm", "tutor", "levelup": MoveFields(draft: $draft, context: context, after: after, before: before, editing: editing)
        case "save", "heal", "blackout":
            Section {
                TextField(kind == "blackout" ? "Black out back to" : (kind == "save" ? "Save location" : "Heal location"), text: strBinding("location"))
            } header: { Text("Location") } footer: { Text("Optional — shown in the event's label.") }
        case "evolve", "change_form": EvolveFields(draft: $draft, context: context)
        case "bag_reorder": BagReorderFields(draft: $draft, context: context, editing: editing)
        case "ev_override": EVOverrideFields(draft: $draft, context: context)
        case "day_care":
            Section("Day Care") {
                AmountStepper(label: "Exp gained", value: intBinding("exp", 1), range: 1...9_999_999, step: 100)
            }
        case "notes":
            if !editing {
                Section("Note") { TextField("Enter note…", text: strBinding("notes"), axis: .vertical).lineLimit(3...10) }
            }
        default: EmptyView()
        }
    }

    private func intBinding(_ k: String, _ d: Int) -> Binding<Int> {
        Binding(get: { draft.i(k, d) }, set: { draft[k] = .number(Double($0)) })
    }
    private func strBinding(_ k: String) -> Binding<String> {
        Binding(get: { draft.s(k) }, set: { draft[k] = .string($0) })
    }
}

struct AmountStepper: View {
    let label: String
    @Binding var value: Int
    var range: ClosedRange<Int> = 1...999
    var step: Int = 1
    @State private var text = ""
    @FocusState private var focused: Bool
    var body: some View {
        HStack {
            Text(label)
            Spacer()
            TextField("", text: $text)
                .keyboardType(.numberPad)
                .multilineTextAlignment(.trailing)
                .monospacedDigit()
                .fontWeight(.semibold)
                .frame(width: 80)
                .focused($focused)
                .onSubmit(commit)
                .onChange(of: focused) { if !focused { commit() } }
            Stepper("", value: Binding(get: { value }, set: { value = min(max($0, range.lowerBound), range.upperBound); text = String(value) }), in: range, step: step)
                .labelsHidden()
        }
        .onAppear { text = String(value) }
        .onChange(of: value) { if !focused { text = String(value) } }
    }
    private func commit() {
        if let v = Int(text.trimmingCharacters(in: .whitespaces)) { value = min(max(v, range.lowerBound), range.upperBound) }
        text = String(value)
    }
}

// MARK: trainer

struct TrainerFields: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    let editing: Bool
    let team: [JSON]
    @State private var locations: [String] = []
    @State private var classes: [String] = []
    @State private var location = ""
    @State private var cls = ""
    @State private var rematches = false
    @State private var picking = false
    @State private var info: JSON = .null

    var body: some View {
        Section {
            Button { picking = true } label: {
                HStack(spacing: 12) {
                    PokemonIcon(species: info.mons.array.last?.name.string, size: 44)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(draft.s("trainer").isEmpty ? "Choose a trainer" : draft.s("trainer")).font(.headline).foregroundStyle(draft.s("trainer").isEmpty ? Color.accentColor : .primary)
                        if !info.isNull {
                            Text("\(info.class.str) · \(info.location.str)").font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    Spacer()
                    Image(systemName: "chevron.right").foregroundStyle(.tertiary)
                }
            }
            .tint(.primary)
            .sheet(isPresented: $picking) {
                TrainerPickerSheet(selection: Binding(get: { draft.s("trainer") }, set: { draft["trainer"] = .string($0); draft["second_trainer"] = nil }), keep: editing ? draft.s("trainer") : nil)
                    .environment(model)
            }
            if !draft.s("second_trainer").isEmpty {
                LabeledContent("Multi partner", value: draft.s("second_trainer"))
            }
            if !info.isNull {
                TrainerTeamStrip(trainer: info)
            }
        } header: { Text("Trainer") }
        .task(id: draft.s("trainer")) {
            info = (await model.run("opts.trainer", ["name": .string(draft.s("trainer"))], quiet: true)) ?? .null
        }
        if editing && !team.isEmpty {
            Section {
                AmountStepper(label: "Pay Day money", value: Binding(get: { draft.i("pay_day") }, set: { draft["pay_day"] = .number(Double($0)) }), range: 0...999_999, step: 10)
            }
            Section {
                ForEach(Array(team.enumerated()), id: \.offset) { i, m in
                    TeamMonEditor(index: i, mon: m, count: team.count, draft: $draft)
                }
            } header: { Text("Team") } footer: { Text("Order is the battle order; Exp split is how many Pokémon share the experience; Thief steals the held item.") }
        }
    }
}

struct TrainerTeamStrip: View {
    let trainer: JSON
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 10) {
                    ForEach(Array(trainer.mons.array.enumerated()), id: \.offset) { _, m in
                        VStack(spacing: 2) {
                            PokemonIcon(species: m.name.string, size: 40)
                            Text("Lv \(m.level.int ?? 0)").font(.caption2.weight(.semibold))
                        }
                    }
                }
            }
            HStack(spacing: 8) {
                Chip(text: "¥\((trainer.money.int ?? 0).grouped)", color: .green)
                Chip(text: "\((trainer.total_exp.int ?? 0).grouped) exp", color: .blue)
                if let e = trainer.exp_per_sec.string, !e.isEmpty { Chip(text: "\(e) xp/s", color: .purple) }
                if trainer["double"].bool { Chip(text: "Double", color: .orange) }
                if trainer.defeated.bool { Chip(text: "Fought", color: .secondary) }
            }
        }
    }
}

struct TeamMonEditor: View {
    let index: Int
    let mon: JSON
    let count: Int
    @Binding var draft: Draft

    private var defIndex: Int { mon.definition_order.int ?? index }

    private func arr(_ k: String, fill: Int) -> [Int] {
        var a = draft[k]?.ints ?? []
        while a.count < count { a.append(fill == -1 ? a.count + 1 : fill) }
        return a
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                PokemonIcon(species: mon.name.string, size: 40)
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text(mon.name.str).font(.headline)
                        Text("Lv \(mon.level.int ?? 0)").font(.subheadline).foregroundStyle(.secondary)
                    }
                    Text([mon.ability.string, mon.nature.string, (mon.xp.int ?? 0) > 0 ? "Exp \(mon.xp.int ?? 0)" : nil].compactMap { $0 }.joined(separator: " · ")).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                ForEach(mon.types.strings, id: \.self) { TypeBadge(type: $0) }
            }
            StatGridMini(stats: mon.stats.array)
            if !mon.moves.strings.isEmpty {
                Text(mon.moves.strings.joined(separator: " · ")).font(.caption).foregroundStyle(.secondary)
            }
            HStack {
                Menu {
                    ForEach(1...count, id: \.self) { n in Button("\(n)") { reorder(to: n) } }
                } label: { Chip(text: "Order \(arr("mon_order", fill: -1)[safe: defIndex] ?? index + 1)", color: .accentColor, symbol: "list.number") }
                Menu {
                    ForEach(1...6, id: \.self) { n in Button("\(n)") { var a = arr("exp_split", fill: 1); a[defIndex] = n; draft["exp_split"] = JSON(a) } }
                } label: { Chip(text: "Exp split \(arr("exp_split", fill: 1)[safe: defIndex] ?? 1)", color: .blue, symbol: "person.2") }
                Spacer()
                if mon.held_item.string != nil {
                    Toggle(isOn: Binding(get: { (draft["thief"]?.ints ?? []).contains(defIndex) }, set: { on in
                        var t = Set(draft["thief"]?.ints ?? [])
                        if on { t.insert(defIndex) } else { t.remove(defIndex) }
                        draft["thief"] = JSON(t.sorted())
                    })) { Text("Thief").font(.caption.weight(.semibold)) }
                    .fixedSize()
                }
            }
            if let h = mon.held_item.string { Text("Holds \(h)").font(.caption2).foregroundStyle(.secondary) }
        }
        .padding(.vertical, 4)
    }

    /// The desktop's reorder rule: the chosen slot is taken, the others keep
    /// their relative order.
    private func reorder(to v: Int) {
        let order = arr("mon_order", fill: -1)
        // the others keep their relative order in the remaining positions
        let others = order.indices.filter { $0 != defIndex }.sorted { order[$0] < order[$1] }
        let positions = (1...count).filter { $0 != v }
        var result = order
        for (k, idx) in others.enumerated() { result[idx] = positions[k] }
        result[defIndex] = v
        draft["mon_order"] = JSON(result)
    }
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}

struct StatGridMini: View {
    let stats: [JSON]
    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(stats.enumerated()), id: \.offset) { i, s in
                VStack(spacing: 1) {
                    Text(s[0].str).font(.system(size: 9, weight: .bold)).foregroundStyle(Theme.statColor(statIndex(s[0].str)))
                    Text("\(s[1].int ?? 0)").font(.caption.weight(.semibold)).monospacedDigit()
                }
                .frame(maxWidth: .infinity)
            }
        }
        .padding(.vertical, 5)
        .background(Theme.well, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
    private func statIndex(_ n: String) -> Int {
        ["HP": 0, "Atk": 1, "Def": 2, "SpA": 3, "Spc": 3, "SpD": 4, "Spe": 5][n] ?? 0
    }
}

/// Location / class filters, rematches, multi partner and the trainer list.
struct TrainerPickerSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @Binding var selection: String
    var keep: String? = nil
    var onPick: ((String) -> Void)? = nil
    @State private var locations: [String] = []
    @State private var classes: [String] = []
    @State private var location = ""
    @State private var cls = ""
    @State private var rematches = false
    @State private var filter = ""
    @State private var trainers: [JSON] = []

    var body: some View {
        NavigationStack {
            List {
                Section {
                    Picker("Location", selection: $location) {
                        Text("All locations").tag("")
                        ForEach(locations, id: \.self) { Text($0).tag($0) }
                    }
                    Picker("Class", selection: $cls) {
                        Text("All classes").tag("")
                        ForEach(classes, id: \.self) { Text($0).tag($0) }
                    }
                    Toggle("Show rematches", isOn: $rematches)
                }
                Section {
                    if trainers.isEmpty { Text("No valid trainers").foregroundStyle(.secondary) }
                    ForEach(trainers.map { Item($0, key: "name") }) { t in
                        Button {
                            selection = t["name"].str
                            onPick?(t["name"].str)
                            Haptics.tap()
                            dismiss()
                        } label: {
                            TrainerListRow(t: t.json, selected: t["name"].str == selection)
                        }
                    }
                } header: {
                    Text("\(trainers.count) trainers")
                } footer: {
                    if model.generation <= 2 { Text("Trainers already fought in this route are hidden.") }
                }
            }
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always), prompt: "Trainer name")
            .navigationTitle("Choose Trainer")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .task {
                locations = (await model.run("opts.locations", quiet: true))?.strings ?? []
                classes = (await model.run("opts.classes", quiet: true))?.strings ?? []
            }
            .task(id: "\(location)|\(cls)|\(rematches)|\(filter)") { await load() }
        }
    }

    private func load() async {
        var p: JSON = ["location": .string(location), "class": .string(cls), "rematches": .bool(rematches), "filter": .string(filter)]
        if let keep { p = ["location": .string(location), "class": .string(cls), "rematches": .bool(rematches), "filter": .string(filter), "keep": .string(keep)] }
        trainers = (await model.run("opts.trainers", p, quiet: true))?.array ?? []
    }
}

struct TrainerListRow: View {
    let t: JSON
    var selected = false
    var body: some View {
        HStack(spacing: 12) {
            PokemonIcon(species: t.lead.string, size: 36)
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(t.name.str).foregroundStyle(.primary)
                    if t.major.bool { Image(systemName: "star.fill").font(.caption2).foregroundStyle(.orange) }
                    if t["double"].bool { Chip(text: "Double", color: .orange) }
                }
                Text("\(t.location.str) · \(t.mons.int ?? 0) Pokémon").font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if let e = t.exp_per_sec.string, !e.isEmpty {
                Text("\(e) xp/s").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
            }
            if selected { Image(systemName: "checkmark").foregroundStyle(.tint) }
        }
    }
}

// MARK: wild

struct WildFields: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    var body: some View {
        Section("Wild Pokémon") {
            NavigationLink {
                SearchPicker(title: "Pokémon", selection: Binding(get: { draft.s("species") }, set: { draft["species"] = .string($0) }), showsPokemon: true) { f in
                    (await model.run("opts.pokemon", ["filter": .string(f)], quiet: true))?.strings ?? []
                }
            } label: {
                HStack(spacing: 12) {
                    PokemonIcon(species: draft.s("species").isEmpty ? nil : draft.s("species"), size: 40)
                    Text(draft.s("species").isEmpty ? "Choose a Pokémon" : draft.s("species")).foregroundStyle(draft.s("species").isEmpty ? .secondary : .primary)
                }
            }
            AmountStepper(label: "Level", value: Binding(get: { draft.i("level", 5) }, set: { draft["level"] = .number(Double($0)) }), range: 2...100)
            AmountStepper(label: "Quantity", value: Binding(get: { draft.i("quantity", 1) }, set: { draft["quantity"] = .number(Double($0)) }), range: 1...999)
            Toggle("Trainer's Pokémon", isOn: Binding(get: { draft.b("trainer_pkmn") }, set: { draft["trainer_pkmn"] = .bool($0) }))
        }
    }
}

struct VitaminFields: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    @State private var options: [String] = []
    var body: some View {
        Section("Vitamin / EV Berry") {
            Picker("Item", selection: Binding(get: { draft.s("vitamin") }, set: { draft["vitamin"] = .string($0) })) {
                ForEach(options, id: \.self) { Text($0).tag($0) }
            }
            AmountStepper(label: "Amount", value: Binding(get: { draft.i("amount", 1) }, set: { draft["amount"] = .number(Double($0)) }), range: 1...999)
        }
        .task {
            options = (await model.run("opts.vitamins", quiet: true))?.strings ?? []
            if draft.s("vitamin").isEmpty, let f = options.first { draft["vitamin"] = .string(f) }
        }
    }
}

// MARK: items

struct ItemFields: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    let context: JSON
    var after: Int?
    var before: Int?
    @State private var itemType = "All Items"
    @State private var mart = "All Items"
    @State private var marts: [String] = []
    @State private var items: [JSON] = []
    @State private var info: JSON = .null
    @State private var picking = false

    private var kind: String { draft.s("kind") }

    var body: some View {
        Section {
            Picker("Item type", selection: $itemType) {
                ForEach(["All Items", "Items in Backpack", "Key Items", "TMs", "Other"], id: \.self) { Text($0).tag($0) }
            }
            if kind == "buy_item" {
                Picker("Mart", selection: $mart) {
                    Text("All marts").tag("All Items")
                    ForEach(marts, id: \.self) { Text($0).tag($0) }
                }
            }
            if kind == "hold_item" {
                Toggle("Hold nothing", isOn: Binding(get: { draft.b("hold_nothing") }, set: { draft["hold_nothing"] = .bool($0) }))
            }
            if !(kind == "hold_item" && draft.b("hold_nothing")) {
                Button { picking = true } label: {
                    HStack {
                        Text("Item").foregroundStyle(.primary)
                        Spacer()
                        Text(draft.s("item").isEmpty ? "Choose" : draft.s("item")).foregroundStyle(.secondary)
                        Image(systemName: "chevron.up.chevron.down").font(.caption).foregroundStyle(.tertiary)
                    }
                }
                .tint(.primary)
                .sheet(isPresented: $picking) {
                    ItemPickerSheet(items: items, current: draft.s("item"), showPrice: kind == "buy_item" ? "buy" : (kind == "sell_item" ? "sell" : nil)) { draft["item"] = .string($0) }
                }
            }
            if kind != "hold_item" {
                AmountStepper(label: "Quantity", value: Binding(get: { draft.i("amount", 1) }, set: { draft["amount"] = .number(Double($0)) }), range: 1...999)
            }
            if kind == "buy_item", let p = info.buy.int {
                LabeledContent("Total cost", value: "¥\((p * draft.i("amount", 1)).grouped)")
            }
            if kind == "sell_item", let p = info.sell.int {
                LabeledContent("Total profit", value: "¥\((p * draft.i("amount", 1)).grouped)")
            }
            if kind == "use_item" && info.pp.bool {
                Toggle("Tossed (no PP effect)", isOn: Binding(get: { draft.b("tossed") }, set: { draft["tossed"] = .bool($0) }))
                if info.needs_target.bool && !draft.b("tossed") {
                    Picker("On move", selection: Binding(get: { draft.s("target_move") }, set: { draft["target_move"] = .string($0) })) {
                        Text("(choose a move)").tag("")
                        ForEach(knownMoves, id: \.self) { Text($0).tag($0) }
                    }
                }
            }
            if kind == "hold_item" {
                Toggle("Consume previously held item", isOn: Binding(get: { draft.b("consume_held") }, set: { draft["consume_held"] = .bool($0) }))
            }
        } header: {
            Text(header)
        }
        .task { marts = (await model.run("opts.marts", quiet: true))?.strings ?? [] }
        .task(id: "\(itemType)|\(mart)") { await loadItems() }
        .task(id: draft.s("item")) {
            info = draft.s("item").isEmpty ? .null : ((await model.run("opts.item", ["name": .string(draft.s("item"))], quiet: true)) ?? .null)
        }
    }

    private var header: String {
        switch kind {
        case "get_item": return "Acquire Item"
        case "buy_item": return "Purchase Item"
        case "sell_item": return "Sell Item"
        case "use_item": return "Use / Drop Item"
        default: return "Hold Item"
        }
    }

    private var knownMoves: [String] {
        var m = context.moves.array.compactMap { $0.string }.filter { !$0.isEmpty }
        let t = draft.s("target_move")
        if !t.isEmpty && !m.contains(t) { m.append(t) }
        return m
    }

    private func loadItems() async {
        var p: JSON = ["type": .string(itemType), "mart": .string(mart)]
        if let after { p = ["type": .string(itemType), "mart": .string(mart), "after": .number(Double(after))] }
        if let before { p = ["type": .string(itemType), "mart": .string(mart), "before": .number(Double(before))] }
        items = (await model.run("opts.items", p, quiet: true))?.array ?? []
    }
}

struct ItemPickerSheet: View {
    let items: [JSON]
    let current: String
    var showPrice: String? = nil
    let onPick: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var filter = ""
    var body: some View {
        NavigationStack {
            List(items.filter { filter.isEmpty || $0.name.str.localizedCaseInsensitiveContains(filter) }.map { Item($0, key: "name") }) { it in
                Button {
                    onPick(it["name"].str)
                    Haptics.tap()
                    dismiss()
                } label: {
                    HStack {
                        Image(systemName: it["key"].bool ? "key.fill" : (it["move"].string != nil ? "opticaldisc.fill" : "bag.fill"))
                            .foregroundStyle(.secondary).frame(width: 24)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(it["name"].str).foregroundStyle(.primary)
                            if let m = it["move"].string { Text(m).font(.caption).foregroundStyle(.secondary) }
                        }
                        Spacer()
                        if let k = showPrice, let p = it[k].int { Text("¥\(p.grouped)").font(.caption.monospacedDigit()).foregroundStyle(.secondary) }
                        if it["name"].str == current { Image(systemName: "checkmark").foregroundStyle(.tint) }
                    }
                }
            }
            .overlay { if items.isEmpty { ContentUnavailableView("No valid items", systemImage: "bag") } }
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle("Choose Item")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
    }
}

// MARK: moves

struct MoveFields: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    let context: JSON
    var after: Int?
    var before: Int?
    let editing: Bool
    @State private var tms: [JSON] = []
    @State private var locked = false

    private var kind: String { draft.s("kind") }

    var body: some View {
        Section {
            if kind == "levelup" {
                LabeledContent("Move", value: draft.s("move"))
                LabeledContent("Learned at", value: "Lv \(draft["level"]?.string ?? "?")")
            } else {
                if editing {
                    Picker("Source", selection: Binding(get: { kind }, set: { draft["kind"] = .string($0) })) {
                        Text("TM / HM").tag("tm")
                        Text("Tutor / Deleter").tag("tutor")
                    }
                    .pickerStyle(.segmented)
                }
                if kind == "tm" {
                    PickerRow(label: "TM / HM", value: draft.s("tm"), options: tms.map { $0.name.str }) { v in
                        draft["tm"] = .string(v)
                        draft["move"] = tms.first { $0.name.str == v }?.move ?? .null
                    }
                    if let m = draft["move"]?.string { LabeledContent("Teaches", value: m) }
                } else {
                    NavigationLink {
                        SearchPicker(title: "Move", selection: Binding(get: { draft.s("move").isEmpty ? "Delete Move" : draft.s("move") }, set: { draft["move"] = .string($0) })) { f in
                            (await model.run("opts.moves", ["filter": .string(f), "include_delete": true], quiet: true))?.strings ?? []
                        }
                    } label: {
                        LabeledContent("Move", value: draft.s("move").isEmpty ? "Choose (or Delete Move)" : draft.s("move"))
                    }
                }
            }
            Picker("Destination", selection: Binding(get: { draft["dest"]?.int ?? -1 }, set: { draft["dest"] = $0 < 0 ? .null : .number(Double($0)) })) {
                Text("Don't learn").tag(-1)
                ForEach(0..<4, id: \.self) { i in
                    Text("Slot \(i + 1) — over \(context.moves[i].string ?? "nothing")").tag(i)
                }
            }
            .disabled(locked && kind != "levelup")
        } header: {
            Text(kind == "levelup" ? "Level-up Move" : (kind == "tm" ? "TM / HM Move" : "Tutor Move"))
        } footer: {
            if locked && kind != "levelup" { Text("The destination follows from the current moveset (already known, or an empty slot).") }
        }
        .task {
            tms = (await model.run("opts.tms", quiet: true))?.array ?? []
            if kind == "tm" && draft.s("tm").isEmpty, let f = tms.first {
                draft["tm"] = f.name
                draft["move"] = f.move
            }
        }
        .task(id: draft.s("move") + kind) { await updateDestination() }
    }

    /// The details editor's destination lock.
    private func updateDestination() async {
        guard kind != "levelup" else { return }
        var p: JSON = ["move": .string(draft.s("move"))]
        if let a = after { p = ["move": .string(draft.s("move")), "after": .number(Double(a))] }
        if let b = before { p = ["move": .string(draft.s("move")), "before": .number(Double(b))] }
        guard let r = await model.run("opts.move_destination", p, quiet: true) else { return }
        locked = !r.free.bool
        if locked || (!editing && draft["dest"] == nil) {
            draft["dest"] = r.dest.int.map { .number(Double($0)) } ?? .null
        }
    }
}

// MARK: evolve

struct EvolveFields: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    let context: JSON
    @State private var stones: [String] = []

    var body: some View {
        Section {
            NavigationLink {
                SearchPicker(title: "Evolves Into", selection: Binding(get: { draft.s("species") }, set: { draft["species"] = .string($0) }), showsPokemon: true) { f in
                    (await model.run("opts.pokemon", ["filter": .string(f), "same_growth_as": .string(context.species.str)], quiet: true))?.strings ?? []
                }
            } label: {
                HStack(spacing: 12) {
                    PokemonIcon(species: context.species.string, size: 36)
                    Image(systemName: "arrow.right").foregroundStyle(.secondary)
                    PokemonIcon(species: draft.s("species").isEmpty ? nil : draft.s("species"), size: 36)
                    Text(draft.s("species").isEmpty ? "Choose species" : draft.s("species"))
                }
            }
            Picker("By stone", selection: Binding(get: { draft.s("stone") }, set: { draft["stone"] = .string($0) })) {
                Text("No stone").tag("")
                ForEach(stones, id: \.self) { Text($0).tag($0) }
            }
            let forms = context.forms.strings
            if !forms.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Change form").font(.caption).foregroundStyle(.secondary)
                    ScrollView(.horizontal, showsIndicators: false) {
                        HStack {
                            ForEach(forms, id: \.self) { f in
                                Button {
                                    draft["species"] = .string(f)
                                } label: {
                                    HStack(spacing: 4) { PokemonIcon(species: f, size: 24); Text(f).font(.caption) }
                                }
                                .buttonStyle(.bordered)
                                .disabled(draft.s("species") == f)
                            }
                        }
                    }
                }
            }
        } header: { Text("Evolution") } footer: { Text("Only species with the same growth rate are listed.") }
        .task {
            stones = (await model.run("opts.stones", quiet: true))?.strings ?? []
            if draft.s("species").isEmpty, draft.s("kind") == "evolve", let sp = context.species.string { draft["species"] = .string(sp) }
            if draft.s("kind") == "change_form", draft.s("species").isEmpty, let f = context.forms.strings.first(where: { $0 != context.species.str }) { draft["species"] = .string(f) }
        }
    }
}

// MARK: bag reorder

struct BagReorderFields: View {
    @Binding var draft: Draft
    let context: JSON
    let editing: Bool
    @State private var order: [String] = []

    var body: some View {
        if !editing {
            Section { Text("Adds a bag reorder here; arrange the bag from the event's screen.").foregroundStyle(.secondary) }
        } else {
            Section {
                if order.isEmpty { Text("The bag is empty before this event.").foregroundStyle(.secondary) }
                ForEach(Array(order.enumerated()), id: \.element) { i, name in
                    HStack {
                        Text(String(format: "%02d", i + 1)).font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                        Text(name)
                    }
                }
                .onMove { from, to in
                    order.move(fromOffsets: from, toOffset: to)
                    draft["bag"] = JSON(order)
                    Haptics.tap()
                }
            } header: {
                HStack { Text("Bag order"); Spacer(); EditButton().font(.caption).textCase(nil) }
            } footer: {
                Text("Drag rows to rearrange the bag.")
            }
            .onAppear {
                if order.isEmpty { order = draft["bag_order"]?.strings ?? context.bag.array.map { $0.name.str } }
            }
        }
    }
}

// MARK: EV override

struct EVOverrideFields: View {
    @Binding var draft: Draft
    let context: JSON
    private var gen: Int { context.generation.int ?? 3 }
    private var cap: Int { context.ev_cap.int ?? 255 }
    private var labels: [(Int, String)] {
        gen <= 2 ? [(0, "HP"), (1, "Attack"), (2, "Defense"), (3, "Special"), (5, "Speed")] : [(0, "HP"), (1, "Attack"), (2, "Defense"), (3, "Sp. Atk"), (4, "Sp. Def"), (5, "Speed")]
    }
    private var values: [Int] {
        let v = draft["evs"]?.ints ?? []
        return v.count == 6 ? v : (context.evs.ints.count == 6 ? context.evs.ints : [0, 0, 0, 0, 0, 0])
    }

    var body: some View {
        Section {
            ForEach(labels, id: \.0) { idx, label in
                HStack {
                    VStack(alignment: .leading) {
                        Text(label)
                        Text("Before: \(context.evs.ints[safe: idx].map { $0.grouped } ?? "—")").font(.caption2).foregroundStyle(.secondary)
                    }
                    AmountStepper(label: "", value: Binding(get: { values[idx] }, set: { v in
                        var a = values
                        a[idx] = v
                        if gen <= 2 && idx == 3 { a[4] = v }
                        draft["evs"] = JSON(a)
                    }), range: 0...cap, step: gen <= 2 ? 256 : 4)
                }
            }
            if gen >= 3 {
                let total = values.reduce(0, +)
                LabeledContent("Total", value: "\(total) / 510").foregroundStyle(total > 510 ? Theme.failure : .primary)
            }
            Button("Use current values") { draft["evs"] = JSON(context.evs.ints) }
        } header: {
            Text("\(gen <= 2 ? "Stat Exp" : "EVs") from this point on (0–\(cap.grouped) per stat)")
        }
    }
}
