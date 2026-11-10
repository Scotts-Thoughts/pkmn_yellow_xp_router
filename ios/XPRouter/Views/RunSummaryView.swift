import SwiftUI

/// The run summary: every major fight (and rare candy use) with the level,
/// held item and moveset at that point, equal neighbours merged.
struct RunSummaryView: View {
    @Environment(AppModel.self) private var model
    @State private var data: JSON = .null
    @State private var shareImage: ExportImage?

    var body: some View {
        Group {
            if data.columns.array.isEmpty && !data.isNull {
                ContentUnavailableView("No major fights", systemImage: "tablecells", description: Text("Add major fights, or highlight other fights, to see the summary."))
            } else {
                ScrollView(.vertical) { ScrollView(.horizontal, showsIndicators: false) { grid.padding() }.frame(maxWidth: .infinity, alignment: .leading) }
            }
        }
        .background(Theme.page)
        .navigationTitle("Run Summary")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button {
                    let r = ImageRenderer(content: grid.padding().background(Theme.page).environment(model))
                    r.scale = 3
                    if let img = r.uiImage { shareImage = ExportImage(image: img) }
                } label: { Image(systemName: "square.and.arrow.up") }
                .disabled(data.columns.array.isEmpty)
            }
        }
        .task(id: model.revision) { data = (await model.run("summary.run")) ?? .null }
        .sheet(item: $shareImage) { e in ShareSheet(items: [e.image]) }
    }

    private var cols: [JSON] { data.columns.array }
    private let w: CGFloat = 112

    private var grid: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 4) {
                ForEach(Array(cols.enumerated()), id: \.offset) { _, c in
                    Button { if let id = c.id.int { model.path.append(.event(id)) } } label: {
                        VStack(spacing: 3) {
                            Text(c.title.str).font(.caption.weight(.bold)).multilineTextAlignment(.center).lineLimit(2)
                            Text(c.level.str).font(.caption2.weight(.semibold)).opacity(0.85)
                        }
                        .foregroundStyle(.white)
                        .frame(width: w, height: 52)
                        .background(c.candy.bool ? Color(hex: "#8a6d12") : (c.category.string != nil ? Theme.categoryColor(c.category.string) : Color(hex: "#737373")), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                    }
                    .buttonStyle(.plain)
                }
            }
            if data.show_held.bool {
                mergedRow(values: cols.map { ($0.held.str, "") }, color: { _ in Color(hex: "#506878") })
            }
            ForEach(0..<4, id: \.self) { slot in
                mergedRow(values: cols.map { ($0.moves[slot].name.str, $0.moves[slot].type.str) }, color: { Theme.typeColor($0) })
            }
        }
    }

    private func mergedRow(values: [(String, String)], color: @escaping (String) -> Color) -> some View {
        var spans: [(String, String, Int)] = []
        for v in values {
            if let last = spans.last, last.0 == v.0 { spans[spans.count - 1].2 += 1 } else { spans.append((v.0, v.1, 1)) }
        }
        return HStack(spacing: 4) {
            ForEach(Array(spans.enumerated()), id: \.offset) { _, s in
                Text(s.0.isEmpty ? "—" : s.0)
                    .font(.caption.weight(.semibold))
                    .lineLimit(1).minimumScaleFactor(0.7)
                    .foregroundStyle(["Poison", "Ghost", "Dark", "Dragon", "Fighting"].contains(s.1) || s.1.isEmpty ? .white : .black)
                    .frame(width: w * CGFloat(s.2) + 4 * CGFloat(s.2 - 1), height: 30)
                    .background(s.0.isEmpty ? Color.secondary.opacity(0.2) : color(s.1), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            }
        }
    }
}
