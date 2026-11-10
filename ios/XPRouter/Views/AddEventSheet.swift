import SwiftUI

/// Add Event: pick a kind, fill its form, insert after (or before) the
/// chosen event. Also the quick-add extras: a whole area's trainers, the
/// Elite Four, and multi battles.
struct AddEventSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    var after: Int?
    var before: Int?

    @State private var kinds: [JSON] = []
    @State private var context: JSON = .null
    @State private var draft: Draft = [:]
    @State private var path: [String] = []
    @State private var anchorLabel = ""
    @State private var league: JSON = .null
    @State private var showArea = false
    @State private var showMulti = false

    private var groups: [String] {
        var seen: [String] = []
        for k in kinds where !seen.contains(k.group.str) { seen.append(k.group.str) }
        return seen
    }

    var body: some View {
        NavigationStack(path: $path) {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    insertionBanner
                    ForEach(groups, id: \.self) { g in
                        VStack(alignment: .leading, spacing: 10) {
                            Text(g.uppercased()).font(.caption.weight(.semibold)).foregroundStyle(.secondary).tracking(0.6)
                            LazyVGrid(columns: [GridItem(.adaptive(minimum: 100), spacing: 10)], spacing: 10) {
                                ForEach(kinds.filter { $0.group.str == g }.map { Item($0, key: "kind") }) { k in
                                    Button {
                                        draft = ["kind": k["kind"]]
                                        path.append(k["kind"].str)
                                    } label: {
                                        KindTile(symbol: k["symbol"].str, label: k["label"].str, color: kindColor(k["kind"].str))
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                    }
                    quickSection
                }
                .padding()
            }
            .background(Theme.page)
            .navigationTitle("Add Event")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .navigationDestination(for: String.self) { kind in
                AddEventForm(draft: $draft, context: context, after: after, before: before, title: kinds.first { $0.kind.str == kind }?.label.str ?? "Event") { id in
                    dismiss()
                    if let id { model.focusedEvent = id }
                }
            }
            .task {
                var p: JSON = [:]
                if let a = after { p = ["after": .number(Double(a))] }
                if let b = before { p = ["before": .number(Double(b))] }
                kinds = (await model.run("opts.kinds", p, quiet: true))?.array ?? []
                context = (await model.run("opts.context", p, quiet: true)) ?? .null
                league = (await model.run("opts.league", quiet: true)) ?? .null
                if let id = after ?? before, let row = model.rows.first(where: { $0.id.int == id }) { anchorLabel = row.name.str }
            }
            .sheet(isPresented: $showArea) { AreaPickerSheet(after: after) { dismiss() } }
            .sheet(isPresented: $showMulti) { MultiBattleSheet(after: after) { dismiss() } }
        }
    }

    private var insertionBanner: some View {
        HStack(spacing: 10) {
            Image(systemName: before != nil ? "arrow.up.to.line" : "arrow.down.to.line").foregroundStyle(.tint)
            VStack(alignment: .leading, spacing: 2) {
                Text(before != nil ? "Insert before" : "Insert after").font(.caption).foregroundStyle(.secondary)
                Text(anchorLabel.isEmpty ? (model.rows.isEmpty ? "Start of the route" : "End of the route") : anchorLabel).font(.subheadline.weight(.semibold)).lineLimit(1)
            }
            Spacer()
            if context.has_state.bool {
                PokemonIcon(species: context.species.string, size: 30)
                Text("Lv \(context.level.int ?? 0)").font(.caption.weight(.semibold))
            }
        }
        .padding(12)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private var quickSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("QUICK ADD").font(.caption.weight(.semibold)).foregroundStyle(.secondary).tracking(0.6)
            VStack(spacing: 0) {
                quickRow("All Trainers in an Area", "map.fill") { showArea = true }
                if model.generation >= 3 {
                    Divider().padding(.leading, 48)
                    quickRow("Multi Battle (Partner + Trainer)", "person.3.fill") { showMulti = true }
                }
                if let e4 = league.elite4.array.first, !e4.isNull {
                    Divider().padding(.leading, 48)
                    quickRow("Elite Four", "crown") { Task { await addLeague(false) } }
                        .help(league.elite4.strings.joined(separator: ", "))
                }
                if !league.with_champion.isNull {
                    Divider().padding(.leading, 48)
                    quickRow("Elite Four + Champion", "crown.fill") { Task { await addLeague(true) } }
                }
            }
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }

    private func quickRow(_ title: String, _ symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Image(systemName: symbol).frame(width: 24).foregroundStyle(.tint)
                Text(title).foregroundStyle(.primary)
                Spacer()
                Image(systemName: "chevron.right").font(.caption).foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 12).padding(.vertical, 13)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func addLeague(_ champ: Bool) async {
        if let r = await model.mutate("event.add_league", ["champion": .bool(champ)]), let id = r.id.int {
            model.focusedEvent = id
            Haptics.success()
            dismiss()
        }
    }
}

func kindColor(_ k: String) -> Color {
    switch k {
    case "trainer": return Color(hex: "#ff6b3d")
    case "wild": return Color(hex: "#34a853")
    case "rare_candy": return Color(hex: "#c47dff")
    case "vitamin", "ev_override": return Color(hex: "#ff4f8b")
    case "tm", "tutor", "levelup": return Color(hex: "#4a8cff")
    case "evolve", "change_form": return Color(hex: "#af52de")
    case "save", "heal", "blackout", "day_care": return Color(hex: "#5ac8fa")
    case "notes": return Color(hex: "#8e8e93")
    default: return Color(hex: "#f5a623")
    }
}

struct KindTile: View {
    let symbol: String
    let label: String
    let color: Color
    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.title3.weight(.semibold))
                .foregroundStyle(.white)
                .frame(width: 42, height: 42)
                .background(color.gradient, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            Text(label).font(.caption.weight(.medium)).multilineTextAlignment(.center).lineLimit(2).minimumScaleFactor(0.85)
                .foregroundStyle(.primary)
        }
        .frame(maxWidth: .infinity, minHeight: 92)
        .padding(.vertical, 8)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}

struct AddEventForm: View {
    @Environment(AppModel.self) private var model
    @Binding var draft: Draft
    let context: JSON
    var after: Int?
    var before: Int?
    let title: String
    let done: (Int?) -> Void
    @State private var adding = false

    var body: some View {
        Form {
            EventForm(draft: $draft, context: context, editing: false, after: after, before: before)
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Add") { Task { await add() } }.fontWeight(.semibold).disabled(adding || !ready)
            }
        }
    }

    private var ready: Bool {
        switch draft.s("kind") {
        case "trainer": return !draft.s("trainer").isEmpty
        case "wild": return !draft.s("species").isEmpty
        case "get_item", "buy_item", "sell_item", "use_item": return !draft.s("item").isEmpty
        case "hold_item": return !draft.s("item").isEmpty || draft.b("hold_nothing")
        case "tm": return !draft.s("tm").isEmpty
        case "vitamin": return !draft.s("vitamin").isEmpty
        default: return true
        }
    }

    private func add() async {
        adding = true
        defer { adding = false }
        var d = draft
        if d.s("kind") == "use_item" { d["convert"] = true }
        var p: JSON = ["draft": .object(d)]
        if let a = after { p = ["draft": .object(d), "after": .number(Double(a)), "into_folder": true] }
        if let b = before { p = ["draft": .object(d), "before": .number(Double(b))] }
        if let r = await model.mutate("event.create", p) {
            Haptics.success()
            done(r.id.int)
        }
    }
}

struct AreaPickerSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    var after: Int?
    let done: () -> Void
    @State private var locations: [String] = []
    @State private var filter = ""
    @State private var rematches = false
    var body: some View {
        NavigationStack {
            List {
                Toggle("Include rematches", isOn: $rematches)
                ForEach(locations.filter { filter.isEmpty || $0.localizedCaseInsensitiveContains(filter) }, id: \.self) { loc in
                    Button(loc) {
                        Task {
                            await model.mutate("event.add_area", ["location": .string(loc), "rematches": .bool(rematches), "after": JSON(after)])
                            Haptics.success()
                            dismiss()
                            done()
                        }
                    }
                }
            }
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle("Add Area")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .task { locations = (await model.run("opts.locations", quiet: true))?.strings ?? [] }
        }
    }
}

/// Gen 3+ multi battles: a partner trainer, then the trainer fought beside them.
struct MultiBattleSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    var after: Int?
    let done: () -> Void
    @State private var partner = ""
    @State private var second = ""
    @State private var pickPartner = false
    @State private var pickSecond = false
    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Button { pickPartner = true } label: { LabeledContent("Partner", value: partner.isEmpty ? "Choose" : partner) }
                    Button { pickSecond = true } label: { LabeledContent("Second trainer", value: second.isEmpty ? "Choose" : second) }.disabled(partner.isEmpty)
                } footer: { Text("Only trainers with up to three Pokémon can take part in a multi battle.") }
            }
            .navigationTitle("Multi Battle")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") {
                        Task {
                            if await model.mutate("event.add_trainer_quick", ["trainer": .string(second), "partner": .string(partner), "after": JSON(after)]) != nil {
                                dismiss(); done()
                            }
                        }
                    }.disabled(partner.isEmpty || second.isEmpty)
                }
            }
            .sheet(isPresented: $pickPartner) { MultiTrainerPicker(selection: $partner, partner: nil) }
            .sheet(isPresented: $pickSecond) { MultiTrainerPicker(selection: $second, partner: partner) }
        }
    }
}

struct MultiTrainerPicker: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @Binding var selection: String
    let partner: String?
    @State private var trainers: [JSON] = []
    @State private var filter = ""
    var body: some View {
        NavigationStack {
            List(trainers.filter { filter.isEmpty || $0.name.str.localizedCaseInsensitiveContains(filter) }.map { Item($0, key: "name") }) { t in
                Button { selection = t["name"].str; dismiss() } label: { TrainerListRow(t: t.json, selected: t["name"].str == selection) }
            }
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle(partner == nil ? "Partner" : "Second Trainer")
            .navigationBarTitleDisplayMode(.inline)
            .task {
                var p: JSON = ["multi_only": true]
                if let partner {
                    let loc = (await model.run("opts.trainer", ["name": .string(partner)], quiet: true))?.location.str ?? ""
                    p = ["multi_only": true, "partner": .string(partner), "location": .string(loc)]
                }
                trainers = (await model.run("opts.trainers", p, quiet: true))?.array ?? []
            }
        }
    }
}
