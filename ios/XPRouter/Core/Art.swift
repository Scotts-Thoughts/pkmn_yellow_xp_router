import SwiftUI
import UIKit

/// Pokémon icons and box art bundled from the desktop's assets
/// (ios/scripts/prepare_assets.sh).
enum Art {
    private static let cache = NSCache<NSString, UIImage>()
    private static let dex: [String: Int] = {
        guard let url = Bundle.main.url(forResource: "dex_lookup", withExtension: "json", subdirectory: "Art"),
              let data = try? Data(contentsOf: url),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        var out: [String: Int] = [:]
        for (k, v) in obj { if let n = v as? Int { out[k] = n } }
        return out
    }()

    static func dexNumber(_ species: String) -> Int? {
        if let n = dex[species] { return n }
        // forms: "Shaymin (Sky)" -> "Shaymin"
        if let base = species.split(separator: "(").first?.trimmingCharacters(in: .whitespaces), let n = dex[base] { return n }
        return dex.first { $0.key.caseInsensitiveCompare(species) == .orderedSame }?.value
    }

    static func pokemon(_ species: String) -> UIImage? {
        guard let n = dexNumber(species) else { return nil }
        return image("pkmn/\(n)")
    }

    static func boxArt(_ version: String) -> UIImage? {
        image("boxart/\(version)")
    }

    private static func image(_ rel: String) -> UIImage? {
        if let c = cache.object(forKey: rel as NSString) { return c }
        guard let url = Bundle.main.url(forResource: rel, withExtension: "png", subdirectory: "Art"),
              let img = UIImage(contentsOfFile: url.path) else { return nil }
        cache.setObject(img, forKey: rel as NSString)
        return img
    }
}

struct PokemonIcon: View {
    let species: String?
    var size: CGFloat = 36
    var body: some View {
        Group {
            if let s = species, let img = Art.pokemon(s) {
                Image(uiImage: img).resizable().interpolation(.high).scaledToFit()
            } else {
                Image(systemName: "questionmark.circle.fill").resizable().scaledToFit().foregroundStyle(.quaternary).padding(size * 0.15)
            }
        }
        .frame(width: size, height: size)
    }
}

struct BoxArt: View {
    let version: String?
    var size: CGFloat = 44
    var body: some View {
        Group {
            if let v = version, let img = Art.boxArt(v) {
                Image(uiImage: img).resizable().interpolation(.high).scaledToFill()
            } else {
                ZStack {
                    LinearGradient(colors: [Theme.versionColor(version ?? ""), Theme.versionColor(version ?? "").opacity(0.6)], startPoint: .topLeading, endPoint: .bottomTrailing)
                    Text(String((version ?? "?").prefix(2))).font(.system(size: size * 0.32, weight: .heavy, design: .rounded)).foregroundStyle(.white)
                }
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: size * 0.18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: size * 0.18, style: .continuous).strokeBorder(.white.opacity(0.12)))
    }
}
