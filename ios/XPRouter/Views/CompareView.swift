import SwiftUI

/// Route comparison: two routes (saved, the open one, or a file) side by
/// side — Overview, Checkpoints and the Event diff.
struct CompareView: View {
    @Environment(AppModel.self) private var model
    let seed: JSON?
    @State private var a: JSON = .null
    @State private var b: JSON = .null
    @State private var result: JSON = .null
    @State private var loading = false
    @State private var error: String?
    @State private var tab = 0
    @State private var picking: Int?
    @State private var majorOnly = true
    @State private var expanded: Int?
    @State private var filtersOn: Set<String> = []
    @State private var highlight = true

    var body: some View {
        ScrollView {
            VStack(spacing: 14) {
                HStack(spacing: 10) {
                    slotButton(0, a)
                    Button { swap(&a, &b); Task { await run() } } label: { Image(systemName: "arrow.left.arrow.right").font(.headline) }
                        .disabled(a.isNull || b.isNull)
                    slotButton(1, b)
                }
                if loading { ProgressView().padding(40) }
                if let error { Banner(text: error, color: Theme.failure, symbol: "xmark.octagon.fill").padding() }
                if !result.isNull && !loading {
                    banners
                    Picker("", selection: $tab) {
                        Text("Overview").tag(0)
                        Text("Checkpoints").tag(1)
                        Text("Event Diff").tag(2)
                    }
                    .pickerStyle(.segmented)
                    switch tab {
                    case 0: overview
                    case 1: checkpoints
                    default: diff
                    }
                } else if !loading && error == nil {
                    ContentUnavailableView("Pick two routes to compare", systemImage: "rectangle.split.2x1", description: Text("Saved routes, or the route open in the editor (unsaved edits included)."))
                }
            }
            .padding()
        }
        .background(Theme.page)
        .navigationTitle("Compare")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if !result.isNull {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { Task { if let t = await model.run("compare.summary_text") { UIPasteboard.general.string = t.str; model.show("Summary copied") } } } label: { Image(systemName: "doc.on.doc") }
                }
            }
        }
        .task {
            if let s = seed {
                a = s.a
                b = s.b
                if a.isNull || b.isNull { picking = a.isNull ? 0 : 1 } else { await run() }
            } else {
                if model.isOpen { a = ["current": true] }
                picking = a.isNull ? 0 : 1
            }
        }
        .sheet(item: Binding(get: { picking.map { PickSide(side: $0) } }, set: { picking = $0?.side })) { p in
            CompareRoutePicker(other: p.side == 0 ? b : a) { choice in
                if p.side == 0 { a = choice } else { b = choice }
                Task { await run() }
            }
        }
    }

    private struct PickSide: Identifiable { let side: Int; var id: Int { side } }

    private func run() async {
        guard !a.isNull, !b.isNull else { return }
        loading = true
        error = nil
        do {
            result = try await Core.shared.call("compare.run", ["a": a, "b": b])
            filtersOn = Set(result.filters.array.filter { $0.on.bool }.map { $0.label.str })
        } catch {
            self.error = error.localizedDescription
            result = .null
        }
        loading = false
    }

    private func label(_ s: JSON) -> String {
        if s.current.bool { return model.routeName.isEmpty ? "Open route" : model.routeName }
        return s.name.string ?? "Choose…"
    }

    private func slotButton(_ side: Int, _ s: JSON) -> some View {
        let d = side == 0 ? result.a : result.b
        return Button { picking = side } label: {
            HStack(spacing: 8) {
                Text(side == 0 ? "A" : "B").font(.caption.weight(.heavy)).foregroundStyle(.white).frame(width: 22, height: 22)
                    .background(side == 0 ? Color.blue : Color.orange, in: Circle())
                VStack(alignment: .leading, spacing: 1) {
                    Text(s.isNull ? "Choose route \(side == 0 ? "A" : "B")" : label(s)).font(.subheadline.weight(.semibold)).lineLimit(1)
                    if !d.isNull { Text("\(d.version.str) · \(d.species.str) · \(d.origin.str)").font(.caption2).foregroundStyle(.secondary).lineLimit(1) }
                }
                Spacer(minLength: 0)
            }
            .padding(10)
            .frame(maxWidth: .infinity)
            .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder private var banners: some View {
        let c = result.compat
        VStack(alignment: .leading, spacing: 6) {
            if !c.same_version.bool { Banner(text: "A is \(result.a.version.str), B is \(result.b.version.str). Trainers are matched by name and team.", color: Theme.warning, symbol: "exclamationmark.triangle.fill") }
            if !c.same_generation.bool { Banner(text: "Different generations: stats, EVs and DVs aren't comparable.", color: Theme.warning, symbol: "exclamationmark.triangle.fill") }
            if !c.same_species.bool { Banner(text: "Different Pokémon: \(result.a.species.str) vs \(result.b.species.str).", color: .secondary, symbol: "info.circle") }
            let errs = result.errors.ints
            if errs.count == 2 && (errs[0] > 0 || errs[1] > 0) { Banner(text: "A has \(errs[0]) events with errors, B has \(errs[1]).", color: Theme.failure, symbol: "xmark.octagon") }
            if c.same_file.bool { Banner(text: "Same route on both sides.", color: .secondary, symbol: "equal.circle") }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    // MARK: overview

    @ViewBuilder private var overview: some View {
        let A = result.a, B = result.b
        HStack(alignment: .top, spacing: 10) {
            identity(A, "A", .blue)
            identity(B, "B", .orange)
        }
        if let d = result.final_time_delta.object["text"]?.string {
            Text("B is \(d) \(result.final_time_delta.faster.bool ? "faster" : "slower") than A").font(.subheadline.weight(.semibold)).foregroundStyle(result.final_time_delta.faster.bool ? Theme.success : Theme.failure)
        }
        if result.compat.same_generation.bool {
            Card(title: "Setup (\(A.dv_text.str))") {
                let names = A.generation.int ?? 3 <= 2 ? ["HP", "Atk", "Def", "Spc", "", "Spe"] : ["HP", "Atk", "Def", "SpA", "SpD", "Spe"]
                compareGrid(rows: (0..<6).filter { !names[$0].isEmpty }.map { (names[$0], A.dvs[$0].int ?? 0, B.dvs[$0].int ?? 0) } + [("Total", A.dv_total.int ?? 0, B.dv_total.int ?? 0)])
                if !A.nature.isNull || !B.nature.isNull { traitRow("Nature", A.nature.str, B.nature.str) }
                if !A.ability.isNull || !B.ability.isNull { traitRow("Ability", A.ability.str, B.ability.str) }
                if !A.hidden_power.isNull { traitRow("Hidden Power", "\(A.hidden_power.type.str) \(A.hidden_power.power.int ?? 0)", "\(B.hidden_power.type.str) \(B.hidden_power.power.int ?? 0)") }
            }
            Card(title: "Final stats & \(A.ev_text.str)") {
                let names = A.generation.int ?? 3 <= 2 ? ["HP", "Atk", "Def", "Spc", "", "Spe"] : ["HP", "Atk", "Def", "SpA", "SpD", "Spe"]
                compareGrid(rows: (0..<6).filter { !names[$0].isEmpty }.map { (names[$0], A.end.stats[$0].int ?? 0, B.end.stats[$0].int ?? 0) })
                Divider()
                compareGrid(rows: (0..<6).filter { !names[$0].isEmpty }.map { ("\(names[$0]) \(A.ev_text.str)", A.end.evs[$0].int ?? 0, B.end.evs[$0].int ?? 0) } + [("Total", A.end.ev_total.int ?? 0, B.end.ev_total.int ?? 0)])
            }
        }
        Card(title: "Where the exp came from") {
            expBar(A, .blue)
            expBar(B, .orange)
            HStack(spacing: 12) {
                legend("Trainers", .red); legend("Wild", .green); legend("Rare Candy", .purple); legend("Other", .gray)
            }
            .font(.caption2)
        }
        Card(title: "Route totals") {
            let groups = ["Battles", "Levelling", "Moves", "Items", "Other"]
            ForEach(groups, id: \.self) { g in
                let rows = zip(A.totals.array, B.totals.array).filter { $0.0[0].str == g }
                let shown = rows.enumerated().filter { $0.offset == 0 || ($0.element.0[2].int ?? 0) != 0 || ($0.element.1[2].int ?? 0) != 0 }
                if !shown.isEmpty {
                    Text(g.uppercased()).font(.caption2.weight(.bold)).foregroundStyle(.secondary).padding(.top, 4)
                    compareGrid(rows: shown.map { ($0.element.0[1].str, $0.element.0[2].int ?? 0, $0.element.1[2].int ?? 0) })
                }
            }
            if !A.vitamins_text.str.isEmpty || !B.vitamins_text.str.isEmpty {
                Text("Vitamins — A: \(A.vitamins_text.str.isEmpty ? "none" : A.vitamins_text.str); B: \(B.vitamins_text.str.isEmpty ? "none" : B.vitamins_text.str)").font(.caption).foregroundStyle(.secondary)
            }
        }
        Card(title: "Money") {
            let rows = zip(A.money.array, B.money.array).filter { ($0.0[1].int ?? 0) != 0 || ($0.1[1].int ?? 0) != 0 }
            compareGrid(rows: rows.map { ($0.0[0].str, $0.0[1].int ?? 0, $0.1[1].int ?? 0) } + [("Final money", A.end.money.int ?? 0, B.end.money.int ?? 0)], money: true)
        }
        Card(title: "Final moves") {
            HStack(alignment: .top) {
                moveColumn(A.end.moves.array, other: B.end.moves.array)
                moveColumn(B.end.moves.array, other: A.end.moves.array)
            }
        }
        trainersCard
        movesLearnedCard
    }

    private func identity(_ d: JSON, _ badge: String, _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                PokemonIcon(species: d.species.string, size: 44)
                Spacer()
                Text(badge).font(.caption.weight(.heavy)).foregroundStyle(.white).frame(width: 22, height: 22).background(color, in: Circle())
            }
            Text(d.species.str).font(.headline)
            Text("Lv \(d.end.level.int ?? 0) · \(d.version.str)").font(.caption).foregroundStyle(.secondary)
            Text("\(d.event_count.int ?? 0) events · \(d.folder_count.int ?? 0) folders").font(.caption2).foregroundStyle(.secondary)
            if let t = d.final_time.string { Text("Final time \(t)").font(.caption2.monospacedDigit()) }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
    }

    private func compareGrid(rows: [(String, Int, Int)], money: Bool = false) -> some View {
        VStack(spacing: 4) {
            HStack {
                Text("").frame(maxWidth: .infinity, alignment: .leading)
                Text("A").frame(width: 64, alignment: .trailing).foregroundStyle(.blue)
                Text("B").frame(width: 64, alignment: .trailing).foregroundStyle(.orange)
                Text("B−A").frame(width: 56, alignment: .trailing)
            }
            .font(.caption2.weight(.bold)).foregroundStyle(.secondary)
            ForEach(Array(rows.enumerated()), id: \.offset) { _, r in
                let d = r.2 - r.1
                HStack {
                    Text(r.0).frame(maxWidth: .infinity, alignment: .leading).lineLimit(1)
                    Text(fmt(r.1, money)).frame(width: 64, alignment: .trailing)
                    Text(fmt(r.2, money)).frame(width: 64, alignment: .trailing)
                    Text(d == 0 ? "—" : (d > 0 ? "+" : "−") + fmt(abs(d), money))
                        .frame(width: 56, alignment: .trailing)
                        .foregroundStyle(d == 0 ? Color.secondary : (d > 0 ? Theme.success : Theme.failure))
                        .fontWeight(highlight && d != 0 ? .semibold : .regular)
                }
                .font(.caption.monospacedDigit())
            }
        }
    }

    private func fmt(_ v: Int, _ money: Bool) -> String { money ? "¥\(v.grouped)" : v.grouped }

    private func traitRow(_ label: String, _ a: String, _ b: String) -> some View {
        HStack {
            Text(label).frame(maxWidth: .infinity, alignment: .leading)
            Text(a.isEmpty ? "—" : a).foregroundStyle(.blue)
            Text("·").foregroundStyle(.secondary)
            Text(b.isEmpty ? "—" : b).foregroundStyle(.orange)
        }
        .font(.caption)
        .fontWeight(a != b && highlight ? .semibold : .regular)
    }

    private func expBar(_ d: JSON, _ color: Color) -> some View {
        let x = d.xp
        let total = max(1, x.total.int ?? 1)
        let parts: [(Int, Color)] = [(x.trainers.int ?? 0, .red), (x.wild.int ?? 0, .green), (x.candies.int ?? 0, .purple), (x.other.int ?? 0, .gray)]
        return VStack(alignment: .leading, spacing: 3) {
            Text("\(color == .blue ? "A" : "B") · \((x.total.int ?? 0).grouped) exp").font(.caption2).foregroundStyle(.secondary)
            GeometryReader { geo in
                HStack(spacing: 1) {
                    ForEach(Array(parts.enumerated()), id: \.offset) { _, p in
                        if p.0 > 0 { Rectangle().fill(p.1.gradient).frame(width: geo.size.width * CGFloat(p.0) / CGFloat(total)) }
                    }
                }
            }
            .frame(height: 12)
            .clipShape(Capsule())
        }
    }

    private func legend(_ t: String, _ c: Color) -> some View { HStack(spacing: 4) { Circle().fill(c).frame(width: 7, height: 7); Text(t) } }

    private func moveColumn(_ moves: [JSON], other: [JSON]) -> some View {
        let otherSet = Set(other.compactMap { $0.string })
        return VStack(alignment: .leading, spacing: 4) {
            ForEach(0..<4, id: \.self) { i in
                let m = moves[safe: i]?.string
                Text(m ?? "—").font(.subheadline.weight(m != nil && !otherSet.contains(m!) ? .bold : .regular))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var trainersCard: some View {
        let ts = result.trainer_sets
        return Card(title: "Trainers") {
            LabeledContent("Fought in both, same order", value: "\(ts.same_order.int ?? 0)")
            LabeledContent("Fought in both, different order", value: "\(ts.different_order.int ?? 0)")
            ForEach([("Only in A", "only_a"), ("Only in B", "only_b"), ("Fought twice in A", "repeat_a"), ("Fought twice in B", "repeat_b")], id: \.1) { label, key in
                let names = ts[key].strings
                if !names.isEmpty {
                    DisclosureGroup("\(label) (\(names.count))") {
                        FlowChips(items: names) { n in
                            tab = 2
                            _ = n
                        }
                    }
                }
            }
        }
        .font(.subheadline)
    }

    private var movesLearnedCard: some View {
        var merged: [String: (String, [Int], [Int], String)] = [:]
        var order: [String] = []
        for m in result.a.moves_learned.array {
            let n = m.name.str
            if merged[n] == nil { order.append(n); merged[n] = (m.source.str, [], [], "") }
            merged[n]!.1.append(m.level.int ?? 0)
        }
        for m in result.b.moves_learned.array {
            let n = m.name.str
            if merged[n] == nil { order.append(n); merged[n] = ("", [], [], m.source.str) }
            merged[n]!.2.append(m.level.int ?? 0)
            if merged[n]!.3.isEmpty { merged[n]!.3 = m.source.str }
        }
        order.sort { (merged[$0]!.1 + merged[$0]!.2).min() ?? 0 < (merged[$1]!.1 + merged[$1]!.2).min() ?? 0 }
        return Card(title: "Moves learned (level)") {
            if order.isEmpty { Text("Neither route learned a move.").foregroundStyle(.secondary) }
            ForEach(order, id: \.self) { n in
                let m = merged[n]!
                let src = m.0.isEmpty ? m.3 : (m.3.isEmpty || m.3 == m.0 ? m.0 : "\(m.0) / \(m.3)")
                HStack {
                    Text(n).frame(maxWidth: .infinity, alignment: .leading)
                    Text(src).foregroundStyle(.secondary).frame(width: 80, alignment: .leading)
                    Text(m.1.isEmpty ? "—" : m.1.map(String.init).joined(separator: ", ")).foregroundStyle(.blue).frame(width: 44, alignment: .trailing)
                    Text(m.2.isEmpty ? "—" : m.2.map(String.init).joined(separator: ", ")).foregroundStyle(.orange).frame(width: 44, alignment: .trailing)
                }
                .font(.caption)
            }
        }
    }

    // MARK: checkpoints

    @ViewBuilder private var checkpoints: some View {
        Picker("", selection: $majorOnly) {
            Text("Major fights").tag(true)
            Text("All shared (\(result.checkpoints.array.count))").tag(false)
        }
        .pickerStyle(.segmented)
        let cps = result.checkpoints.array.enumerated().filter { !majorOnly || $0.element.a.major.bool || $0.element.b.major.bool }
        if cps.isEmpty {
            ContentUnavailableView("No checkpoints", systemImage: "flag", description: Text(majorOnly ? "No major fight in common. Switch to all shared trainers." : "These routes have no trainer fights in common."))
        }
        ForEach(cps, id: \.offset) { i, cp in
            Button { withAnimation { expanded = expanded == i ? nil : i } } label: {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Circle().fill(Theme.categoryColor(cp.a.category.string)).frame(width: 8, height: 8)
                        Text(cp.a.label.str).font(.subheadline.weight(.semibold)).lineLimit(1)
                        if !cp.same_order.bool { Chip(text: "moved", color: .orange) }
                        Spacer()
                        Text(cp.moves_differing.int.map { $0 == 0 ? "moves same" : "\($0) moves differ" } ?? "").font(.caption2).foregroundStyle(.secondary)
                    }
                    HStack(spacing: 14) {
                        stat("Lv", cp.a.before.level.int, cp.b.before.level.int)
                        stat("¥", cp.a.before.money.int, cp.b.before.money.int)
                        stat("EVs", cp.a.before.ev_total.int, cp.b.before.ev_total.int)
                        if result.both_have_times.bool, let d = cp.time_delta.string { Text("A vs B \(d)").font(.caption2).foregroundStyle(cp.b_ahead.bool ? Theme.success : Theme.failure) }
                    }
                    if expanded == i {
                        HStack(alignment: .top) {
                            moveColumn(cp.a.before.moves.array, other: cp.b.before.moves.array)
                            moveColumn(cp.b.before.moves.array, other: cp.a.before.moves.array)
                        }
                        let names = ["HP", "Atk", "Def", "SpA", "SpD", "Spe"]
                        compareGrid(rows: (0..<6).map { (names[$0], cp.a.before.stats[$0].int ?? 0, cp.b.before.stats[$0].int ?? 0) })
                    }
                }
                .padding(12)
                .background(Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            }
            .buttonStyle(.plain)
        }
    }

    private func stat(_ l: String, _ a: Int?, _ b: Int?) -> some View {
        HStack(spacing: 3) {
            Text(l).foregroundStyle(.secondary)
            Text("\(a ?? 0)").foregroundStyle(.blue)
            Text("/").foregroundStyle(.tertiary)
            Text("\(b ?? 0)").foregroundStyle(.orange)
        }
        .font(.caption.monospacedDigit())
    }

    // MARK: diff

    @ViewBuilder private var diff: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack {
                ForEach(result.filters.array.map { Item($0, key: "label") }) { f in
                    let on = filtersOn.contains(f["label"].str)
                    Button { if on { filtersOn.remove(f["label"].str) } else { filtersOn.insert(f["label"].str) } } label: {
                        Chip(text: f["label"].str, color: .accentColor, filled: on)
                    }
                }
            }
        }
        Text("\(result.shared_fights.int ?? 0) shared fights · \(result.blocks_with_differences.int ?? 0) blocks with differences").font(.caption).foregroundStyle(.secondary)
        LazyVStack(spacing: 6) {
            ForEach(Array(result.rows.array.enumerated()), id: \.offset) { _, r in
                if r.anchor.bool {
                    HStack {
                        Image(systemName: "equal.circle.fill").foregroundStyle(.secondary)
                        VStack(alignment: .leading) {
                            Text(r.a.label.str).font(.subheadline.weight(.semibold))
                            HStack {
                                Text("Lv \(r.a.before.level.int ?? 0)").foregroundStyle(.blue)
                                Text("Lv \(r.b.before.level.int ?? 0)").foregroundStyle(.orange)
                                if let d = r.time_delta.string { Text(d).foregroundStyle(.secondary) }
                            }
                            .font(.caption)
                        }
                        Spacer()
                    }
                    .padding(10)
                    .background(Theme.card, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                } else {
                    let la = r.a.array.filter { filtersOn.contains($0.filter.str) }
                    let lb = r.b.array.filter { filtersOn.contains($0.filter.str) }
                    if !la.isEmpty || !lb.isEmpty {
                        HStack(alignment: .top, spacing: 8) {
                            lane(la, .blue)
                            lane(lb, .orange)
                        }
                    }
                }
            }
        }
    }

    private func lane(_ items: [JSON], _ color: Color) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            ForEach(Array(items.enumerated()), id: \.offset) { _, it in
                let only = ["only", "moved", "repeat"].contains(it.status.str)
                HStack(alignment: .top, spacing: 4) {
                    if only && highlight { Rectangle().fill(color).frame(width: 3) }
                    VStack(alignment: .leading, spacing: 1) {
                        HStack(spacing: 4) {
                            Text(it.kind.str).font(.system(size: 8, weight: .heavy)).foregroundStyle(.secondary)
                            Text(it.label.str).font(.caption.weight(only ? .semibold : .regular)).foregroundStyle(only ? Color.primary : Color.secondary)
                        }
                        if it.status.str == "qty" { Text("×\(it.qty.int ?? 0) vs ×\(it.other_qty.int ?? 0)").font(.caption2).foregroundStyle(.orange) }
                        else if (it.qty.int ?? 1) > 1 { Text("×\(it.qty.int ?? 0)").font(.caption2).foregroundStyle(.secondary) }
                        if it.status.str == "moved" { Text(it.other_after.string.map { "moved: other route fights them after \($0)" } ?? "moved: at the start").font(.caption2).foregroundStyle(.secondary) }
                        if it.status.str == "repeat" { Text("rematch").font(.caption2).foregroundStyle(.secondary) }
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(8)
        .background(Theme.card.opacity(0.7), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

struct FlowChips: View {
    let items: [String]
    let tap: (String) -> Void
    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 120), spacing: 6)], alignment: .leading, spacing: 6) {
            ForEach(items, id: \.self) { n in
                Button { tap(n) } label: { Chip(text: n, color: .secondary) }.buttonStyle(.plain)
            }
        }
    }
}

struct CompareRoutePicker: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let other: JSON
    let onPick: (JSON) -> Void
    @State private var lib: JSON = .null
    @State private var search = ""
    @State private var importing = false
    var body: some View {
        NavigationStack {
            List {
                if model.isOpen && !other.current.bool {
                    Button { onPick(["current": true]); dismiss() } label: { Label("The route open in the editor", systemImage: "pencil.and.list.clipboard") }
                }
                Button { importing = true } label: { Label("A route file…", systemImage: "folder") }
                Section("Saved routes") {
                    ForEach(lib.routes.array.map { Item($0, key: "name") }) { r in
                        Button { onPick(["name": r["name"]]); dismiss() } label: { RouteRow(route: r.json) }
                            .buttonStyle(.plain)
                            .disabled(other.name.string == r["name"].str)
                    }
                }
            }
            .searchable(text: $search)
            .navigationTitle("Choose Route")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .task(id: search) { lib = (await model.run("library.list", ["search": .string(search)], quiet: true)) ?? .null }
            .fileImporter(isPresented: $importing, allowedContentTypes: [.json]) { r in
                if case .success(let url) = r {
                    _ = url.startAccessingSecurityScopedResource()
                    // copy out of the provider's sandbox so the core can read it
                    let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(url.lastPathComponent)
                    try? FileManager.default.removeItem(at: tmp)
                    try? FileManager.default.copyItem(at: url, to: tmp)
                    url.stopAccessingSecurityScopedResource()
                    onPick(["path": .string(tmp.path)])
                    dismiss()
                }
            }
        }
    }
}
