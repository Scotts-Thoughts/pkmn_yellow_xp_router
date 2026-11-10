import SwiftUI

/// One event: what the solo Pokémon looks like before it (stats, moves with
/// PP, bag), the event's editor and notes, and its battle summary.
struct EventDetailView: View {
    @Environment(AppModel.self) private var model
    let eventID: Int
    @State private var id: Int = 0
    @State private var detail: JSON = .null
    @State private var draft: Draft = [:]
    @State private var loadedDraft: Draft = [:]
    @State private var notes = ""
    @State private var tab = 0
    @State private var saveTask: Task<Void, Never>?
    @State private var assignSlot: SlotRequest?
    @State private var evOverride = false
    @State private var addTarget: AddTarget?
    @State private var bagExpanded = false

    private var hasBattle: Bool { detail.has_battle.bool }
    private var navIDs: [Int] { model.rows.filter { $0.kind.str != "folder" }.compactMap { $0.id.int } }

    var body: some View {
        Group {
            if tab == 1 && hasBattle {
                BattleView(eventID: detail.battle_id.int ?? id) { slot in assignSlot = SlotRequest(slot: slot, battle: true) }
            } else {
                stateList
            }
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            if hasBattle {
                Picker("", selection: $tab) {
                    Text("Pre-Event State").tag(0)
                    Text("Battle Summary").tag(1)
                }
                .pickerStyle(.segmented)
                .padding(.horizontal).padding(.vertical, 8)
                .background(.bar)
            }
        }
        .navigationTitle(detail.title.string ?? "")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button { step(-1) } label: { Image(systemName: "chevron.up") }.accessibilityLabel("Previous Event").disabled(navIDs.first == id)
                Button { step(1) } label: { Image(systemName: "chevron.down") }.accessibilityLabel("Next Event").disabled(navIDs.last == id)
                Menu {
                    Button { addTarget = AddTarget(after: id) } label: { Label("Add Event After", systemImage: "plus") }
                    Button { addTarget = AddTarget(before: id) } label: { Label("Add Event Before", systemImage: "plus.square.dashed") }
                    if detail.mappable.bool {
                        Button { model.path.append(.map(MapRequest(focusEvent: id))) } label: { Label("Show on Map", systemImage: "map") }
                    }
                    if detail.kind.str == "group" {
                        Button { Task { await model.mutate("edit.set_enabled", ["ids": [.number(Double(id))], "enabled": .bool(!detail.enabled.bool)]); await load() } } label: {
                            Label(detail.enabled.bool ? "Disable" : "Enable", systemImage: detail.enabled.bool ? "eye.slash" : "eye")
                        }
                        HighlightMenu(ids: [id])
                        Button { Task { await model.mutate("edit.move_up", ["ids": [.number(Double(id))]]); await load() } } label: { Label("Move Up", systemImage: "arrow.up") }
                        Button { Task { await model.mutate("edit.move_down", ["ids": [.number(Double(id))]]); await load() } } label: { Label("Move Down", systemImage: "arrow.down") }
                        if detail.can_split.bool {
                            Button { Task { await model.mutate("folder.split", ["id": .number(Double(id))]) } } label: { Label("Split Folder Here", systemImage: "scissors") }
                        }
                        Divider()
                        Button(role: .destructive) {
                            Task {
                                let next = navIDs.first { $0 > id }
                                await model.mutate("edit.delete", ["ids": [.number(Double(id))]])
                                if model.path.last == .event(eventID) { model.path.removeLast() }
                                _ = next
                            }
                        } label: { Label("Delete Event", systemImage: "trash") }
                    }
                } label: { Image(systemName: "ellipsis.circle") }.accessibilityLabel("More")
            }
        }
        .task { if id == 0 { id = eventID }; await load(initial: true) }
        .onChange(of: model.revision) { Task { await load() } }
        .onChange(of: draft) { _, new in scheduleSave(new) }
        .onDisappear { flush() }
        .sheet(item: $assignSlot) { req in
            MoveAssignSheet(slot: req.slot) { mv in
                Task {
                    if req.battle {
                        await model.run("battle.action", ["op": "assign_move", "slot": .number(Double(req.slot)), "value": .string(mv)])
                        await model.routeChanged()
                    } else {
                        await model.mutate("event.assign_move_before", ["id": .number(Double(id)), "slot": .number(Double(req.slot)), "move": .string(mv)])
                    }
                }
            }
        }
        .sheet(isPresented: $evOverride) {
            EVOverrideSheet(context: detail.context) { evs in
                Task { await model.mutate("event.override_evs_before", ["id": .number(Double(id)), "evs": JSON(evs)]) }
            }
        }
        .sheet(item: $addTarget) { t in AddEventSheet(after: t.after, before: t.before) }
    }

    private var stateList: some View {
        List {
            if !detail.errors.array.isEmpty || !detail.warnings.array.isEmpty {
                Section {
                    ForEach(detail.errors.strings, id: \.self) { Banner(text: $0, color: Theme.failure, symbol: "xmark.octagon.fill") }
                    ForEach(detail.warnings.strings, id: \.self) { Banner(text: $0, color: Theme.warning, symbol: "exclamationmark.triangle.fill") }
                }
            }
            if !detail.state.isNull {
                Section {
                    IdentityCard(state: detail.state, label: detail.label.str, location: detail.location.string, after: detail.after)
                }
                Section {
                    StatsCard(state: detail.state) { evOverride = true }
                } header: { Text("Stats") }
                Section {
                    MovesCard(state: detail.state, onAssign: { assignSlot = SlotRequest(slot: $0, battle: false) }, onUsePP: { item, target in
                        Task { await model.mutate("event.use_pp_item_before", ["id": .number(Double(id)), "item": .string(item), "target": JSON(target)]) }
                    })
                } header: { Text("Moves") } footer: { Text("Tap a move to replace it before this event; long-press for PP items.") }
                Section {
                    BagCard(state: detail.state, expanded: $bagExpanded)
                } header: { Text("Bag") }
            }
            if detail.kind.str == "folder" {
                Section { LabeledContent("Folder", value: detail.folder_name.str) }
            } else if detail.editable.bool && draft.s("kind") != "notes" {
                EventForm(draft: $draft, context: detail.context, editing: true, before: id, team: detail.team.array)
                    .disabled(!detail.editable.bool)
            } else if !detail.editable.bool && detail.kind.str == "item" {
                Section { Text("Part of a multi-part event: edit the parent event.").foregroundStyle(.secondary) }
            }
            Section {
                TextField("No notes for this event", text: $notes, axis: .vertical)
                    .lineLimit(2...12)
                    .onChange(of: notes) { scheduleNotes() }
            } header: { Text("Notes") }
        }
        .listStyle(.insetGrouped)
        .listSectionSpacing(.compact)
        .scrollDismissesKeyboard(.interactively)
    }

    private func step(_ d: Int) {
        flush()
        guard let i = navIDs.firstIndex(of: id) else { return }
        let n = i + d
        guard navIDs.indices.contains(n) else { return }
        id = navIDs[n]
        model.focusedEvent = id
        Haptics.tap()
        Task {
            await model.run("tree.select", ["ids": [.number(Double(id))]], quiet: true)
            await load(initial: true)
        }
    }

    private func load(initial: Bool = false) async {
        guard let d = await model.run("event.detail", ["id": .number(Double(id))], quiet: true) else {
            if model.path.last == .event(eventID) { model.path.removeLast() }
            return
        }
        withAnimation(.default) { detail = d }
        let newDraft = d.draft.object
        if initial || newDraft.s("kind") != draft.s("kind") || saveTask == nil {
            loadedDraft = newDraft
            draft = newDraft
        }
        if initial { notes = d.notes.str }
        if initial && hasBattle && (model.settings.auto_switch.bool || model.settings.isNull) { tab = 1 }
        if !hasBattle { tab = 0 }
    }

    private func scheduleSave(_ new: Draft) {
        guard new != loadedDraft, detail.editable.bool else { return }
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(450))
            if Task.isCancelled { return }
            await save(new)
        }
    }

    private func save(_ d: Draft) async {
        var send = d
        send["notes"] = .string(notes)
        send.removeValue(forKey: "bag_order")
        loadedDraft = d
        saveTask = nil
        if let r = await model.run("event.update", ["id": .number(Double(id)), "draft": .object(send)]), r.changed.bool {
            await model.routeChanged()
        }
    }

    @State private var notesTask: Task<Void, Never>?
    private func scheduleNotes() {
        notesTask?.cancel()
        let text = notes
        notesTask = Task {
            try? await Task.sleep(for: .milliseconds(700))
            if Task.isCancelled { return }
            if text != detail.notes.str {
                await model.run("event.set_notes", ["id": .number(Double(id)), "notes": .string(text)])
                await model.refreshState()
                await model.refreshRows()
            }
        }
    }

    private func flush() {
        if let t = saveTask { t.cancel(); let d = draft; Task { await save(d) } }
        if let n = notesTask {
            n.cancel()
            let text = notes, eid = id
            if text != detail.notes.str { Task { await model.run("event.set_notes", ["id": .number(Double(eid)), "notes": .string(text)]); await model.routeChanged() } }
        }
    }
}

struct SlotRequest: Identifiable {
    let slot: Int
    let battle: Bool
    var id: Int { slot * 2 + (battle ? 1 : 0) }
}

struct Banner: View {
    let text: String
    let color: Color
    let symbol: String
    var body: some View {
        Label { Text(text).font(.subheadline) } icon: { Image(systemName: symbol).foregroundStyle(color) }
    }
}

struct IdentityCard: View {
    let state: JSON
    let label: String
    let location: String?
    let after: JSON
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 14) {
                PokemonIcon(species: state.species.string, size: 64)
                    .background(Circle().fill(Theme.typeColor(state.types[0].string).opacity(0.18)))
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Text(state.species.str).font(.title3.weight(.bold))
                        Text("Lv \(state.level.int ?? 0)").font(.subheadline.weight(.heavy)).padding(.horizontal, 8).padding(.vertical, 2)
                            .background(Color.accentColor.opacity(0.15), in: Capsule()).foregroundStyle(.tint)
                    }
                    HStack(spacing: 4) { ForEach(state.types.strings, id: \.self) { TypeBadge(type: $0) } }
                    let meta = [state.ability.string, state.nature.string, state.held_item.string.map { "Held: \($0)" }].compactMap { $0 }
                    if !meta.isEmpty { Text(meta.joined(separator: " · ")).font(.caption).foregroundStyle(.secondary).lineLimit(2) }
                }
            }
            VStack(alignment: .leading, spacing: 4) {
                ProgressView(value: max(0, min(1, 1 - Double(state.pct_to_next.int ?? 0) / 100)))
                    .tint(Color.accentColor)
                HStack {
                    Text("Exp \((state.xp.int ?? 0).grouped)").font(.caption.monospacedDigit())
                    Spacer()
                    Text((state.to_next.int ?? 0) > 0 ? "\((state.to_next.int ?? 0).grouped) to Lv \((state.level.int ?? 0) + 1)" : "Max level").font(.caption.monospacedDigit()).foregroundStyle(.secondary)
                }
            }
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("BEFORE").font(.caption2.weight(.bold)).foregroundStyle(.secondary).tracking(0.5)
                    Text(label).font(.subheadline.weight(.semibold)).foregroundStyle(.orange)
                    if let location { Text(location).font(.caption).foregroundStyle(.secondary) }
                }
                Spacer()
                if let lv = after.level.int {
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("AFTER").font(.caption2.weight(.bold)).foregroundStyle(.secondary).tracking(0.5)
                        Text("Lv \(lv)").font(.subheadline.weight(.semibold))
                        Text("¥\((after.money.int ?? 0).grouped)").font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}

struct StatsCard: View {
    let state: JSON
    let onEVs: () -> Void
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("").frame(maxWidth: .infinity, alignment: .leading)
                Text("Now").frame(width: 52, alignment: .trailing)
                Text("from \(state.ev_label.str)").frame(width: 84, alignment: .trailing)
                Text("\(state.ev_label.str)").frame(width: 92, alignment: .trailing)
            }
            .font(.caption2.weight(.semibold)).foregroundStyle(.secondary)
            .padding(.bottom, 6)
            ForEach(Array(state.stats.array.enumerated()), id: \.offset) { i, s in
                HStack {
                    HStack(spacing: 6) {
                        Circle().fill(Theme.statColor(statIdx(s.name.str))).frame(width: 6, height: 6)
                        Text(s.name.str)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    HStack(spacing: 3) {
                        if s.boosted.bool { Circle().fill(.orange).frame(width: 5, height: 5) }
                        Text("\(s.now.int ?? 0)").fontWeight(.bold).monospacedDigit()
                    }
                    .frame(width: 52, alignment: .trailing)
                    Text(signed(s.gain.int ?? 0)).monospacedDigit().foregroundStyle((s.gain.int ?? 0) > 0 ? Theme.success : .secondary).frame(width: 84, alignment: .trailing)
                    Button(action: onEVs) {
                        Text("\((s.realized.int ?? 0).grouped) / \((s.total.int ?? 0).grouped)").monospacedDigit().lineLimit(1).minimumScaleFactor(0.7)
                    }
                    .buttonStyle(.borderless)
                    .frame(width: 92, alignment: .trailing)
                }
                .font(.subheadline)
                .padding(.vertical, 5)
                if i < state.stats.array.count - 1 { Divider() }
            }
            HStack {
                Text(state.badge_boost.bool ? "• Badge boost applied" : "").foregroundStyle(.orange)
                Spacer()
                Text("Badges: \(state.badges.str)").foregroundStyle(.secondary)
            }
            .font(.caption)
            .padding(.top, 6)
        }
    }
    private func signed(_ v: Int) -> String { v > 0 ? "+\(v)" : "\(v)" }
    private func statIdx(_ n: String) -> Int {
        switch n { case "HP": return 0; case "Attack": return 1; case "Defense": return 2; case "Sp. Atk", "Special": return 3; case "Sp. Def": return 4; default: return 5 }
    }
}

struct MovesCard: View {
    let state: JSON
    let onAssign: (Int) -> Void
    let onUsePP: (String, String?) -> Void
    var body: some View {
        VStack(spacing: 8) {
            ForEach(state.moves.array.map { Item($0, key: "slot") }) { m in
                let name = m["name"].string
                Button { onAssign(m["slot"].int ?? 0) } label: {
                    HStack(spacing: 10) {
                        Text("\((m["slot"].int ?? 0) + 1)").font(.caption.weight(.bold)).foregroundStyle(.secondary).frame(width: 14)
                        RoundedRectangle(cornerRadius: 3).fill(Theme.typeColor(m["type"].string)).frame(width: 5, height: 28)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(name ?? "—").font(.body.weight(name == nil ? .regular : .semibold)).foregroundStyle(name == nil ? .secondary : .primary)
                            if let t = m["type"].string { Text(t).font(.caption2).foregroundStyle(Theme.typeColor(t)) }
                        }
                        Spacer()
                        if let cur = m["pp_cur"].int, let mx = m["pp_max"].int {
                            PPView(cur: cur, max: mx, ups: m["pp_ups"].int ?? 0, spend: m["spend"].int ?? 0)
                        }
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .contextMenu {
                    Button { onAssign(m["slot"].int ?? 0) } label: { Label("Replace Move…", systemImage: "arrow.triangle.2.circlepath") }
                    ForEach(state.pp_items.array.map { Item($0, key: "name") }) { it in
                        if it["needs_target"].bool {
                            if let name { Button("Use \(it["name"].str) on \(name)") { onUsePP(it["name"].str, name) } }
                        } else {
                            Button("Use \(it["name"].str)") { onUsePP(it["name"].str, nil) }
                        }
                    }
                    if !m["history"].array.isEmpty {
                        Section("PP since last full") {
                            ForEach(Array(m["history"].array.enumerated()), id: \.offset) { _, h in
                                Text(h.amount.int.map { "\($0 > 0 ? "+" : "")\($0)  \(h.text.str)" } ?? h.text.str)
                            }
                        }
                    }
                }
            }
        }
    }
}

struct PPView: View {
    let cur: Int
    let max: Int
    let ups: Int
    let spend: Int
    var body: some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text("\(cur < 0 ? "−\(-cur)" : "\(cur)")/\(max) PP")
                .font(.caption.weight(.semibold).monospacedDigit())
                .foregroundStyle(cur <= 0 ? Theme.failure : (spend > cur ? Theme.warning : .secondary))
            HStack(spacing: 2) {
                ForEach(0..<3, id: \.self) { i in Circle().fill(i < ups ? Color.accentColor : Color.secondary.opacity(0.25)).frame(width: 4, height: 4) }
            }
        }
    }
}

struct BagCard: View {
    let state: JSON
    @Binding var expanded: Bool
    var body: some View {
        let bag = state.bag.array
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Label("¥\((state.money.int ?? 0).grouped)", systemImage: "yensign.circle.fill").font(.headline).foregroundStyle(.green)
                Spacer()
                if let lim = state.bag_limit.int { Text("\(bag.count) of \(lim) slots").font(.caption).foregroundStyle(.secondary) }
            }
            if bag.isEmpty {
                Text("Bag is empty before this event").font(.subheadline).foregroundStyle(.secondary)
            }
            ForEach(Array((expanded ? bag : Array(bag.prefix(8))).enumerated()), id: \.offset) { i, it in
                HStack {
                    Text(String(format: "%02d", i + 1)).font(.caption.monospacedDigit()).foregroundStyle(.tertiary)
                    Text(it.name.str)
                    Spacer()
                    Text("×\(it.num.int ?? 0)").monospacedDigit().foregroundStyle(.secondary)
                }
                .font(.subheadline)
            }
            if bag.count > 8 {
                Button(expanded ? "Show less" : "Show all \(bag.count) items") { withAnimation { expanded.toggle() } }.font(.subheadline)
            }
        }
    }
}

/// The searchable move list for "Assign Move to Slot N".
struct MoveAssignSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let slot: Int
    let onPick: (String) -> Void
    @State private var moves: [String] = []
    @State private var filter = ""
    var body: some View {
        NavigationStack {
            List(moves.filter { filter.isEmpty || $0.localizedCaseInsensitiveContains(filter) }, id: \.self) { m in
                Button(m) { onPick(m); Haptics.tap(); dismiss() }.foregroundStyle(.primary)
            }
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle("Assign Move to Slot \(slot + 1)")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .task { moves = (await model.run("opts.moves", quiet: true))?.strings ?? [] }
        }
    }
}

struct EVOverrideSheet: View {
    @Environment(\.dismiss) private var dismiss
    let context: JSON
    let onApply: ([Int]) -> Void
    @State private var draft: Draft = ["kind": "ev_override"]
    var body: some View {
        NavigationStack {
            Form {
                EVOverrideFields(draft: $draft, context: context)
                Section { Text("Adds an EV Override event right before this event.").font(.footnote).foregroundStyle(.secondary) }
            }
            .navigationTitle("Override EVs")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Add") { onApply(draft["evs"]?.ints ?? context.evs.ints); dismiss() }.fontWeight(.semibold)
                }
            }
        }
    }
}
