import SwiftUI

/// A searchable list that picks one string; the options come from the core
/// (filtered there, like the desktop's filter boxes).
struct SearchPicker: View {
    let title: String
    @Binding var selection: String
    var showsPokemon = false
    var subtitle: ((String) -> String?)? = nil
    let provider: (String) async -> [String]
    @Environment(\.dismiss) private var dismiss
    @State private var filter = ""
    @State private var options: [String] = []

    var body: some View {
        List {
            ForEach(options, id: \.self) { o in
                Button {
                    selection = o
                    Haptics.tap()
                    dismiss()
                } label: {
                    HStack(spacing: 12) {
                        if showsPokemon { PokemonIcon(species: o, size: 34) }
                        VStack(alignment: .leading, spacing: 2) {
                            Text(o).foregroundStyle(.primary)
                            if let s = subtitle?(o) { Text(s).font(.caption).foregroundStyle(.secondary) }
                        }
                        Spacer()
                        if o == selection { Image(systemName: "checkmark").foregroundStyle(.tint).fontWeight(.semibold) }
                    }
                }
            }
        }
        .overlay {
            if options.isEmpty { ContentUnavailableView.search(text: filter) }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
        .task(id: filter) {
            try? await Task.sleep(for: .milliseconds(filter.isEmpty ? 0 : 120))
            options = await provider(filter)
        }
    }
}

/// A searchable picker over a fixed list, presented as a sheet.
struct ListPickerSheet: View {
    let title: String
    let options: [String]
    var current: String? = nil
    var showsPokemon = false
    let onPick: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var filter = ""

    private var shown: [String] {
        filter.isEmpty ? options : options.filter { $0.localizedCaseInsensitiveContains(filter) }
    }

    var body: some View {
        NavigationStack {
            List(shown, id: \.self) { o in
                Button {
                    onPick(o)
                    dismiss()
                } label: {
                    HStack(spacing: 12) {
                        if showsPokemon { PokemonIcon(species: o, size: 30) }
                        Text(o.isEmpty ? "None" : o).foregroundStyle(o.isEmpty ? .secondary : .primary)
                        Spacer()
                        if o == current { Image(systemName: "checkmark").foregroundStyle(.tint) }
                    }
                }
            }
            .searchable(text: $filter, placement: .navigationBarDrawer(displayMode: .always))
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
        .presentationDetents([.medium, .large])
    }
}

/// A form row that opens a ListPickerSheet.
struct PickerRow: View {
    let label: String
    let value: String
    let options: [String]
    var showsPokemon = false
    var placeholder = "Choose"
    let onPick: (String) -> Void
    @State private var open = false
    var body: some View {
        Button { open = true } label: {
            HStack {
                Text(label).foregroundStyle(.primary)
                Spacer()
                if showsPokemon && !value.isEmpty { PokemonIcon(species: value, size: 24) }
                Text(value.isEmpty ? placeholder : value).foregroundStyle(.secondary).lineLimit(1)
                Image(systemName: "chevron.up.chevron.down").font(.caption).foregroundStyle(.tertiary)
            }
        }
        .tint(.primary)
        .sheet(isPresented: $open) {
            ListPickerSheet(title: label, options: options, current: value, showsPokemon: showsPokemon, onPick: onPick)
        }
    }
}
