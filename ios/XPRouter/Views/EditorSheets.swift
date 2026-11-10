import SwiftUI

/// The desktop's filter bar: event types (and major battles / invalid
/// events) to show.
struct FilterSheet: View {
    @Binding var filters: JSON
    let apply: ([String]) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var active = Set<String>()

    var body: some View {
        NavigationStack {
            ScrollView {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 10)], spacing: 10) {
                    ForEach(filters.filters.array.map { Item($0, key: "type") }) { f in
                        let on = active.contains(f["type"].str)
                        Button {
                            if on { active.remove(f["type"].str) } else { active.insert(f["type"].str) }
                            Haptics.tap()
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: f["symbol"].str).frame(width: 22)
                                Text(f["label"].str).font(.subheadline.weight(.medium)).lineLimit(1).minimumScaleFactor(0.8)
                                Spacer(minLength: 0)
                            }
                            .padding(12)
                            .foregroundStyle(on ? .white : .primary)
                            .background(on ? Color.accentColor : Theme.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding()
                Text("Common: trainers, rare candies and vitamins.").font(.footnote).foregroundStyle(.secondary)
                Button("Show Common") {
                    active = ["Fight Trainer", "Use Rare Candy", "Use Vitamin"]
                }
                .buttonStyle(.bordered)
                .padding(.bottom)
            }
            .background(Theme.page)
            .navigationTitle("Filter Events")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Clear") { active = []; apply([]); dismiss() } }
                ToolbarItem(placement: .confirmationAction) { Button("Apply") { apply(Array(active)); dismiss() }.fontWeight(.semibold) }
            }
            .onAppear { active = Set(filters.filters.array.filter { $0.active.bool }.map { $0.type.str }) }
        }
        .presentationDetents([.medium, .large])
    }
}

struct TransferSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let ids: [Int]
    @State private var folders: [String] = []
    @State private var newName = ""
    @State private var filter = ""

    var body: some View {
        NavigationStack {
            List {
                Section("New folder") {
                    HStack {
                        TextField("Folder name", text: $newName)
                        Button("Create & Move") {
                            Task {
                                if await model.mutate("edit.transfer", ["ids": JSON(ids), "folder": .string(newName), "create": true]) != nil { dismiss() }
                            }
                        }
                        .disabled(newName.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                }
                Section("Existing folder") {
                    ForEach(folders.filter { filter.isEmpty || $0.localizedCaseInsensitiveContains(filter) }, id: \.self) { f in
                        Button {
                            Task { if await model.mutate("edit.transfer", ["ids": JSON(ids), "folder": .string(f)]) != nil { dismiss() } }
                        } label: { Label(f, systemImage: "folder") }
                    }
                }
            }
            .searchable(text: $filter)
            .navigationTitle("Transfer \(ids.count) Event\(ids.count == 1 ? "" : "s")")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
            .task { folders = (await model.run("folder.destinations", ["ids": JSON(ids)]))?.strings.filter { $0 != "ROOT" } ?? [] }
        }
    }
}

struct SetupSummarySheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    var body: some View {
        NavigationStack {
            ScrollView {
                Text(text).font(.system(.body, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding()
            }
            .navigationTitle("Setup Summary")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
                ToolbarItem(placement: .topBarLeading) { ShareLink(item: text) { Image(systemName: "square.and.arrow.up") } }
            }
            .task(id: model.revision) { text = (await model.run("summary.setup"))?.str ?? "" }
        }
        .presentationDetents([.medium, .large])
    }
}

struct NotesExportSheet: View {
    let text: String
    let url: URL
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            ScrollView {
                Text(text).font(.system(.footnote, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading).padding()
            }
            .navigationTitle("Route Notes")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
                ToolbarItem(placement: .topBarLeading) { ShareLink(item: url) { Image(systemName: "square.and.arrow.up") } }
            }
        }
    }
}

/// File ▸ Customize DVs for the open route.
struct CustomDVSheet: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var dv = DVState()
    var body: some View {
        NavigationStack {
            Form {
                Section {
                    HStack(spacing: 12) {
                        PokemonIcon(species: model.app.species.string, size: 48)
                        VStack(alignment: .leading) {
                            Text(model.app.species.str).font(.headline)
                            Text(model.app.version.str).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                }
                DVSections(dv: $dv, version: model.app.version.str, species: model.app.species.str, fromRoute: true)
            }
            .navigationTitle(model.generation <= 2 ? "Custom DVs" : "Custom IVs")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Apply") {
                        Task {
                            await model.mutate("route.set_dvs", ["dvs": dv.dvsJSON, "ability_idx": .number(Double(dv.ability)), "nature": .number(Double(dv.nature))])
                            dismiss()
                        }
                    }
                    .fontWeight(.semibold)
                }
            }
        }
    }
}
