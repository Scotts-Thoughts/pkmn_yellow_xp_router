import SwiftUI
import UIKit

enum Theme {
    static let accent = Color.accentColor
    static let success = Color(hex: "#34c759")
    static let warning = Color(hex: "#ff9f0a")
    static let failure = Color(hex: "#ff453a")
    static let card = Color(uiColor: .secondarySystemGroupedBackground)
    static let well = Color(uiColor: .tertiarySystemGroupedBackground)
    static let page = Color(uiColor: .systemGroupedBackground)

    /// The desktop's Pokémon type palette.
    static func typeColor(_ t: String?) -> Color {
        let hex: String
        switch t ?? "" {
        case "Normal": hex = "#a8a878"
        case "Fighting": hex = "#c03028"
        case "Grass": hex = "#78c850"
        case "Fire": hex = "#f08030"
        case "Water": hex = "#6890f0"
        case "Electric": hex = "#e8c020"
        case "Ground": hex = "#d8b058"
        case "Rock": hex = "#b8a038"
        case "Psychic": hex = "#f85888"
        case "Poison": hex = "#a040a0"
        case "Flying": hex = "#a890f0"
        case "Bug": hex = "#a8b820"
        case "Ice": hex = "#78c8c8"
        case "Ghost": hex = "#705898"
        case "Dragon": hex = "#7038f8"
        case "Steel": hex = "#a8a8c0"
        case "Dark": hex = "#705848"
        case "Fairy": hex = "#ee99ac"
        case "Curse": hex = "#2e9fa3"
        default: hex = "#8e8e93"
        }
        return Color(hex: hex)
    }

    static func versionColor(_ v: String) -> Color {
        switch v {
        case "Red", "FireRed": return Color(hex: "#e3350d")
        case "Blue": return Color(hex: "#3b6fd4")
        case "Yellow": return Color(hex: "#e6b800")
        case "Gold", "HeartGold": return Color(hex: "#c9a227")
        case "Silver", "SoulSilver": return Color(hex: "#9ea7b3")
        case "Crystal": return Color(hex: "#5fb4d6")
        case "Ruby": return Color(hex: "#c4153b")
        case "Sapphire": return Color(hex: "#2047b8")
        case "Emerald": return Color(hex: "#1f9e57")
        case "LeafGreen": return Color(hex: "#4caf3d")
        case "Diamond": return Color(hex: "#7aa7e0")
        case "Pearl": return Color(hex: "#d98fa8")
        case "Platinum": return Color(hex: "#8a8f9c")
        case "Black", "Black 2": return Color(hex: "#3a3a3a")
        case "White", "White 2": return Color(hex: "#bdbdbd")
        default: return Color(hex: "#8e6bd9")
        }
    }

    static func categoryColor(_ c: String?) -> Color {
        switch c ?? "" {
        case "rival": return Color(hex: "#5e6ad2")
        case "gym_leader": return Color(hex: "#ff9f0a")
        case "elite_four": return Color(hex: "#bf5af2")
        case "champion": return Color(hex: "#30d158")
        case "post_game": return Color(hex: "#64d2ff")
        case "boss": return Color(hex: "#8e8e93")
        case "team_leader": return Color(hex: "#ff375f")
        default: return .secondary
        }
    }

    /// Move highlight states (the desktop's green / blue / orange).
    static func highlight(_ n: Int) -> Color? {
        switch n {
        case 1: return Color(hex: "#1f9e57")
        case 2: return Color(hex: "#2f6fe0")
        case 3: return Color(hex: "#ff8c00")
        default: return nil
        }
    }

    static func statColor(_ i: Int) -> Color {
        [Color(hex: "#ff3b30"), Color(hex: "#f08030"), Color(hex: "#e8b400"), Color(hex: "#6890f0"), Color(hex: "#58b040"), Color(hex: "#f85888")][min(max(i, 0), 5)]
    }
}

extension Color {
    init(hex: String) {
        var s = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("#") { s.removeFirst() }
        var v: UInt64 = 0
        Scanner(string: s).scanHexInt64(&v)
        let r, g, b: Double
        if s.count == 6 {
            r = Double((v >> 16) & 0xff) / 255; g = Double((v >> 8) & 0xff) / 255; b = Double(v & 0xff) / 255
        } else {
            r = 0.5; g = 0.5; b = 0.5
        }
        self = Color(.sRGB, red: r, green: g, blue: b, opacity: 1)
    }

    var hexString: String {
        let c = UIColor(self)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        c.getRed(&r, green: &g, blue: &b, alpha: &a)
        return String(format: "#%02x%02x%02x", Int(max(0, min(1, r)) * 255), Int(max(0, min(1, g)) * 255), Int(max(0, min(1, b)) * 255))
    }
}

enum Haptics {
    static func tap() { UIImpactFeedbackGenerator(style: .light).impactOccurred() }
    static func success() { UINotificationFeedbackGenerator().notificationOccurred(.success) }
    static func warn() { UINotificationFeedbackGenerator().notificationOccurred(.warning) }
}

/// A rounded card section.
struct Card<Content: View>: View {
    var title: String? = nil
    var symbol: String? = nil
    var trailing: AnyView? = nil
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let title {
                HStack(spacing: 6) {
                    if let symbol { Image(systemName: symbol).foregroundStyle(.secondary).font(.subheadline) }
                    Text(title.uppercased()).font(.caption.weight(.semibold)).foregroundStyle(.secondary).tracking(0.6)
                    Spacer()
                    if let trailing { trailing }
                }
            }
            content
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.card, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
    }
}

struct Chip: View {
    let text: String
    var color: Color = .secondary
    var filled = false
    var symbol: String? = nil
    var body: some View {
        HStack(spacing: 4) {
            if let symbol { Image(systemName: symbol).font(.caption2.weight(.bold)) }
            Text(text).font(.caption.weight(.semibold)).lineLimit(1)
        }
        .padding(.horizontal, 8).padding(.vertical, 4)
        .foregroundStyle(filled ? .white : color)
        .background(filled ? color : color.opacity(0.15), in: Capsule())
    }
}

struct TypeBadge: View {
    let type: String
    var body: some View {
        Text(type.uppercased()).font(.system(size: 9, weight: .heavy, design: .rounded))
            .padding(.horizontal, 6).padding(.vertical, 2)
            .foregroundStyle(.white)
            .background(Theme.typeColor(type), in: Capsule())
    }
}

extension Int {
    var grouped: String {
        let f = NumberFormatter()
        f.numberStyle = .decimal
        return f.string(from: NSNumber(value: self)) ?? String(self)
    }
}
