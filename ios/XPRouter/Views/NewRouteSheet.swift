import SwiftUI

/// "Create New Route": game, solo Pokémon, base route, DVs/IVs, ability,
/// nature and a name.
struct NewRouteSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    var onCreated: () -> Void

    @State private var games: [JSON] = []
    @State private var version = "Yellow"
    @State private var species = ""
    @State private var base = "Empty Route"
    @State private var name = ""
    @State private var dv = DVState()
    @State private var creating = false

    private var defaultName: String {
        "\(version.lowercased().replacingOccurrences(of: " ", with: ""))-\(species.lowercased())"
    }

    private var selectedGame: JSON? { games.first { $0.version.str == version } }

    var body: some View {
        NavigationStack {
            Form {
                Section("Game") {
                    GamePickerStrip(games: games, selection: $version)
                        .listRowInsets(EdgeInsets(top: 10, leading: 0, bottom: 10, trailing: 0))
                }
                Section("Solo Pokémon") {
                    NavigationLink {
                        SearchPicker(title: "Solo Pokémon", selection: $species, showsPokemon: true) { filter in
                            (await model.run("newroute.species", ["version": .string(version), "filter": .string(filter)], quiet: true))?.strings ?? []
                        }
                    } label: {
                        HStack(spacing: 12) {
                            PokemonIcon(species: species.isEmpty ? nil : species, size: 44)
                            VStack(alignment: .leading) {
                                Text(species.isEmpty ? "Choose a Pokémon" : species).font(.headline).foregroundStyle(species.isEmpty ? .secondary : .primary)
                                Text("The Pokémon you'll solo the game with").font(.caption).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
                Section {
                    NavigationLink {
                        SearchPicker(title: "Base Route", selection: $base) { filter in
                            (await model.run("newroute.bases", ["version": .string(version), "filter": .string(filter)], quiet: true))?.strings ?? []
                        }
                    } label: {
                        LabeledContent("Base route", value: base)
                    }
                } footer: {
                    Text("Copies the events of a preset or saved route of the same game.")
                }
                DVSections(dv: $dv, version: version, species: species)
                Section {
                    TextField(species.isEmpty ? "Route name" : defaultName, text: $name)
                        .textInputAutocapitalization(.never)
                } header: {
                    Text("Name")
                } footer: {
                    if model.isOpen && model.app.unsaved.bool {
                        Label("Unsaved changes in the open route will be lost.", systemImage: "exclamationmark.triangle.fill").foregroundStyle(Theme.warning)
                    }
                }
            }
            .navigationTitle("New Route")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Create") { Task { await create() } }
                        .fontWeight(.semibold)
                        .disabled(species.isEmpty || creating)
                }
            }
            .task {
                games = (await model.run("newroute.games", quiet: true))?.array ?? []
                if let v = model.app.version.string, games.contains(where: { $0.version.str == v }) { version = v }
            }
            .onChange(of: version) { _, _ in
                species = ""
                base = "Empty Route"
            }
        }
    }

    private func create() async {
        creating = true
        defer { creating = false }
        let r = await model.run("newroute.create", [
            "version": .string(version), "species": .string(species), "base": .string(base),
            "dvs": dv.dvsJSON, "ability_idx": .number(Double(dv.ability)), "nature": .number(Double(dv.nature)),
            "name": .string(name.trimmingCharacters(in: .whitespaces).isEmpty ? defaultName : name),
        ])
        if r != nil {
            Haptics.success()
            dismiss()
            onCreated()
        }
    }
}

struct GamePickerStrip: View {
    let games: [JSON]
    @Binding var selection: String
    var body: some View {
        ScrollViewReader { proxy in
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 12) {
                    ForEach(games.map { Item($0, key: "version") }) { g in
                        let sel = g["version"].str == selection
                        Button {
                            withAnimation(.snappy) { selection = g["version"].str }
                            Haptics.tap()
                        } label: {
                            VStack(spacing: 6) {
                                BoxArt(version: g["art"].string, size: 72)
                                    .overlay(RoundedRectangle(cornerRadius: 13, style: .continuous).strokeBorder(sel ? Color.accentColor : .clear, lineWidth: 3))
                                    .scaleEffect(sel ? 1.0 : 0.92)
                                Text(g["version"].str).font(.caption.weight(sel ? .bold : .regular)).lineLimit(1)
                                Text(g["custom"].bool ? "Custom · Gen \(g["generation"].int ?? 0)" : "Gen \(g["generation"].int ?? 0)")
                                    .font(.caption2).foregroundStyle(.secondary)
                            }
                            .frame(width: 84)
                        }
                        .buttonStyle(.plain)
                        .id(g["version"].str)
                    }
                }
                .padding(.horizontal, 16)
            }
            .onAppear { proxy.scrollTo(selection, anchor: .center) }
        }
    }
}

/// The DV / IV, ability and nature fields (also the route's "Custom DVs").
struct DVState: Equatable {
    var values: [String: Int] = [:]
    var ability = 0
    var nature = 0
    var loadedFor = ""
    var dvsJSON: JSON {
        .object(values.mapValues { .number(Double($0)) })
    }
}

let dvStatOrder: [(String, String)] = [("hp", "HP"), ("attack", "Attack"), ("defense", "Defense"), ("special_attack", "Sp. Atk"), ("special_defense", "Sp. Def"), ("speed", "Speed")]

struct DVSections: View {
    @Environment(AppModel.self) private var model
    @Binding var dv: DVState
    let version: String
    let species: String
    /// start from the open route's values (Custom DVs) instead of the defaults
    var fromRoute = false
    @State private var form: JSON = .null

    private var gen: Int { form.generation.int ?? 3 }
    private var maxV: Int { form.max.int ?? 31 }

    var body: some View {
        Group { sections }
            .task(id: version + "|" + species) { await load() }
    }

    @ViewBuilder private var sections: some View {
        Section {
            ForEach(dvStatOrder, id: \.0) { key, label in
                if gen <= 2 && key == "special_defense" {
                    EmptyView()
                } else {
                    let derived = gen <= 2 && key == "hp"
                    Stepper(value: Binding(get: { value(key) }, set: { dv.values[key] = $0; Task { await refresh() } }), in: 0...maxV) {
                        HStack {
                            Text(gen <= 2 && key == "special_attack" ? "Special" : label)
                            Spacer()
                            Text("\(value(key))").monospacedDigit().fontWeight(.semibold).foregroundStyle(derived ? .secondary : .primary)
                        }
                    }
                    .disabled(derived)
                }
            }
            if let hp = form.hidden_power.object["type"]?.string {
                LabeledContent("Hidden Power") {
                    HStack(spacing: 6) {
                        TypeBadge(type: hp)
                        Text("\(form.hidden_power.power.int ?? 0)").monospacedDigit()
                    }
                }
            }
            HStack {
                Button("All Max") { setAll(maxV) }
                Spacer()
                Button("All 0") { setAll(0) }
            }
            .buttonStyle(.borderless)
            .font(.subheadline)
        } header: {
            Text(form.unit.string.map { "\($0)s" } ?? "DVs / IVs")
        } footer: {
            if gen <= 2 { Text("The HP DV follows from the others in gens 1 and 2.") }
        }
        if form.show_ability.bool || form.show_nature.bool {
            Section("Ability & Nature") {
                if form.show_ability.bool {
                    Picker("Ability", selection: $dv.ability) {
                        ForEach(Array(form.abilities.strings.enumerated()), id: \.offset) { i, a in Text(a).tag(i) }
                    }
                }
                if form.show_nature.bool {
                    Picker("Nature", selection: $dv.nature) {
                        ForEach(form.natures.array.map { Item($0, key: "index") }) { n in
                            Text(natureLabel(n.json)).tag(n["index"].int ?? 0)
                        }
                    }
                    NatureStatGrid(natures: form.natures.array, nature: $dv.nature)
                }
            }
        }
    }

    private func natureLabel(_ n: JSON) -> String {
        guard let up = n.up.string, let down = n.down.string else { return n.name.str }
        return "\(n.name.str)  (+\(short(up)) −\(short(down)))"
    }

    private func value(_ key: String) -> Int {
        if gen <= 2 && key == "hp" {
            let a = dv.values["attack"] ?? 15, d = dv.values["defense"] ?? 15, s = dv.values["speed"] ?? 15, c = dv.values["special_attack"] ?? 15
            return (a % 2) * 8 + (d % 2) * 4 + (s % 2) * 2 + (c % 2)
        }
        return dv.values[key] ?? maxV
    }

    private func setAll(_ v: Int) {
        for (k, _) in dvStatOrder { dv.values[k] = v }
        Task { await refresh() }
    }

    private func refresh() async {
        var p: JSON = ["version": .string(version), "species": .string(species), "dvs": dv.dvsJSON, "nature": .number(Double(dv.nature)), "ability_idx": .number(Double(dv.ability))]
        if version.isEmpty { p = [:] }
        if let f = await model.run("newroute.dv_form", p, quiet: true) { form = f }
    }

    private func load() async {
        let key = version + "|" + species
        guard dv.loadedFor != key else { await refresh(); return }
        let f: JSON?
        if fromRoute && dv.loadedFor.isEmpty {
            f = await model.run("route.dvs", quiet: true)
        } else {
            f = await model.run("newroute.dv_form", ["version": .string(version), "species": .string(species)], quiet: true)
        }
        guard let f else { return }
        let keepValues = !dv.loadedFor.isEmpty && dv.loadedFor.split(separator: "|").first.map(String.init) == version
        form = f
        if !keepValues {
            var vals: [String: Int] = [:]
            for (k, v) in f.dvs.object { vals[k] = v.int ?? 0 }
            dv.values = vals
            dv.nature = f.nature.int ?? 0
        }
        dv.ability = fromRoute && dv.loadedFor.isEmpty ? (f.ability_idx.int ?? 0) : min(dv.ability, max(0, f.abilities.array.count - 1))
        dv.loadedFor = key
    }
}

private func short(_ stat: String) -> String {
    switch stat {
    case "attack": return "Atk"
    case "defense": return "Def"
    case "speed": return "Spe"
    case "special_attack": return "SpA"
    case "special_defense": return "SpD"
    default: return stat
    }
}

/// The desktop's Increase / Decrease buttons, kept in sync with the picker.
struct NatureStatGrid: View {
    let natures: [JSON]
    @Binding var nature: Int
    private let stats = ["attack", "defense", "speed", "special_attack", "special_defense"]
    private let neutral: [String: String] = ["attack": "Hardy", "defense": "Docile", "speed": "Serious", "special_attack": "Bashful", "special_defense": "Quirky"]

    private var current: JSON? { natures.first { $0.index.int == nature } }
    private var up: String? { current?.up.string ?? neutralStat }
    private var down: String? { current?.down.string ?? neutralStat }
    private var neutralStat: String? { neutral.first { $0.value == current?.name.str }?.key }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            row("Raises", selected: up, color: Color(hex: "#ef4444")) { pick(up: $0, down: down) }
            row("Lowers", selected: down, color: Color(hex: "#3b82f6")) { pick(up: up, down: $0) }
        }
        .padding(.vertical, 4)
    }

    private func row(_ title: String, selected: String?, color: Color, action: @escaping (String) -> Void) -> some View {
        HStack(spacing: 6) {
            Text(title).font(.caption).foregroundStyle(.secondary).frame(width: 46, alignment: .leading)
            ForEach(stats, id: \.self) { s in
                Button { action(s) } label: {
                    Text(short(s)).font(.caption.weight(.semibold)).frame(maxWidth: .infinity).padding(.vertical, 6)
                        .background(selected == s ? color : Color.secondary.opacity(0.12), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .foregroundStyle(selected == s ? .white : .primary)
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func pick(up: String?, down: String?) {
        guard let up, let down else { return }
        if up == down {
            if let n = natures.first(where: { $0.name.str == neutral[up] }) { nature = n.index.int ?? 0 }
        } else if let n = natures.first(where: { $0.up.string == up && $0.down.string == down }) {
            nature = n.index.int ?? 0
        }
        Haptics.tap()
    }
}
