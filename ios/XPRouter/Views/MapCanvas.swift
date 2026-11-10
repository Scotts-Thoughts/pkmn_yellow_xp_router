import SwiftUI
import UIKit

/// What the canvas draws on top of the map pictures.
struct MapOverlayData {
    var objects: [MapMarker] = []
    var focus: [CGPoint] = []
    var focusRect: CGRect? = nil
    var trip: [(CGPoint, String)] = []
    var selected: Int? = nil
}

struct MapMarker {
    let index: Int
    let kind: String
    let point: CGPoint
    let routed: Bool
}

/// The map picture: a UIScrollView zooming a CATiledLayer whose tiles are
/// the core's 512 px LOD chunks, plus a marker layer redrawn as you pan.
struct MapCanvas: UIViewRepresentable {
    let key: String            // game|scope|night: rebuilds the tiles when it changes
    let params: JSON           // version / scope / night for chunk requests
    let worldSize: CGSize
    let maxLevel: Int
    let fit: CGRect
    var overlay: MapOverlayData
    var focusTarget: (CGPoint, CGFloat, Int)?   // point, zoom, token
    let onTap: (CGPoint) -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UIView {
        let host = UIView()
        host.backgroundColor = UIColor(red: 0.04, green: 0.04, blue: 0.09, alpha: 1)
        let sv = UIScrollView()
        sv.delegate = context.coordinator
        sv.bouncesZoom = true
        sv.showsHorizontalScrollIndicator = false
        sv.showsVerticalScrollIndicator = false
        sv.contentInsetAdjustmentBehavior = .never
        sv.decelerationRate = .normal
        sv.translatesAutoresizingMaskIntoConstraints = false
        host.addSubview(sv)
        let markers = MarkerView()
        markers.isUserInteractionEnabled = false
        markers.backgroundColor = .clear
        markers.translatesAutoresizingMaskIntoConstraints = false
        host.addSubview(markers)
        NSLayoutConstraint.activate([
            sv.leadingAnchor.constraint(equalTo: host.leadingAnchor), sv.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            sv.topAnchor.constraint(equalTo: host.topAnchor), sv.bottomAnchor.constraint(equalTo: host.bottomAnchor),
            markers.leadingAnchor.constraint(equalTo: host.leadingAnchor), markers.trailingAnchor.constraint(equalTo: host.trailingAnchor),
            markers.topAnchor.constraint(equalTo: host.topAnchor), markers.bottomAnchor.constraint(equalTo: host.bottomAnchor),
        ])
        let tap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.tapped(_:)))
        sv.addGestureRecognizer(tap)
        let dbl = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.doubleTapped(_:)))
        dbl.numberOfTapsRequired = 2
        sv.addGestureRecognizer(dbl)
        tap.require(toFail: dbl)
        context.coordinator.scroll = sv
        context.coordinator.markers = markers
        return host
    }

    func updateUIView(_ host: UIView, context: Context) {
        let c = context.coordinator
        c.parent = self
        if c.key != key {
            c.key = key
            c.rebuild()
        }
        c.markers?.data = overlay
        c.markers?.setNeedsDisplay()
        if let f = focusTarget, f.2 != c.focusToken {
            c.focusToken = f.2
            DispatchQueue.main.async { c.focus(on: f.0, zoom: f.1) }
        }
    }

    final class Coordinator: NSObject, UIScrollViewDelegate {
        var parent: MapCanvas
        var key = ""
        var focusToken = -1
        weak var scroll: UIScrollView?
        weak var markers: MarkerView?
        var tiled: TiledMapView?

        init(_ p: MapCanvas) { parent = p }

        func rebuild() {
            guard let sv = scroll else { return }
            tiled?.removeFromSuperview()
            let size = parent.worldSize
            let v = TiledMapView(frame: CGRect(origin: .zero, size: size))
            v.params = parent.params
            v.maxLevel = parent.maxLevel
            (v.layer as? CATiledLayer)?.levelsOfDetail = max(1, parent.maxLevel + 3)
            (v.layer as? CATiledLayer)?.levelsOfDetailBias = 3
            (v.layer as? CATiledLayer)?.tileSize = CGSize(width: 512, height: 512)
            sv.addSubview(v)
            tiled = v
            sv.contentSize = size
            sv.zoomScale = 1
            DispatchQueue.main.async { self.fitToContent() }
        }

        func fitToContent() {
            guard let sv = scroll, sv.bounds.width > 0 else { return }
            let size = parent.worldSize
            let minScale = min(sv.bounds.width / size.width, sv.bounds.height / size.height)
            sv.minimumZoomScale = max(minScale * 0.9, 0.005)
            sv.maximumZoomScale = 6
            let f = parent.fit.isEmpty ? CGRect(origin: .zero, size: size) : parent.fit
            let s = min(sv.bounds.width / f.width, sv.bounds.height / f.height)
            sv.zoomScale = min(max(s, sv.minimumZoomScale), 2)
            center(on: CGPoint(x: f.midX, y: f.midY))
            markers?.transformInfo = (sv.contentOffset, sv.zoomScale)
            markers?.setNeedsDisplay()
        }

        func center(on p: CGPoint) {
            guard let sv = scroll else { return }
            let z = sv.zoomScale
            var off = CGPoint(x: p.x * z - sv.bounds.width / 2, y: p.y * z - sv.bounds.height / 2)
            let maxX = max(sv.contentSize.width - sv.bounds.width, 0), maxY = max(sv.contentSize.height - sv.bounds.height, 0)
            off.x = min(max(off.x, -sv.contentInset.left), maxX + sv.contentInset.right)
            off.y = min(max(off.y, -sv.contentInset.top), maxY + sv.contentInset.bottom)
            sv.setContentOffset(off, animated: false)
            centerContent()
        }

        func focus(on p: CGPoint, zoom: CGFloat) {
            guard let sv = scroll else { return }
            UIView.animate(withDuration: 0.45, delay: 0, options: [.curveEaseInOut]) {
                sv.zoomScale = min(max(zoom, sv.minimumZoomScale), sv.maximumZoomScale)
                self.center(on: p)
            } completion: { _ in self.syncMarkers() }
        }

        func viewForZooming(in scrollView: UIScrollView) -> UIView? { tiled }
        func scrollViewDidZoom(_ scrollView: UIScrollView) { centerContent(); syncMarkers() }
        func scrollViewDidScroll(_ scrollView: UIScrollView) { syncMarkers() }

        /// Keep a map smaller than the screen centred.
        func centerContent() {
            guard let sv = scroll else { return }
            let w = sv.contentSize.width, h = sv.contentSize.height
            let ix = max((sv.bounds.width - w) / 2, 0), iy = max((sv.bounds.height - h) / 2, 0)
            sv.contentInset = UIEdgeInsets(top: iy, left: ix, bottom: iy, right: ix)
        }

        func syncMarkers() {
            guard let sv = scroll else { return }
            markers?.transformInfo = (sv.contentOffset, sv.zoomScale)
            markers?.setNeedsDisplay()
        }

        @objc func tapped(_ g: UITapGestureRecognizer) {
            guard let v = tiled else { return }
            let p = g.location(in: v)
            parent.onTap(p)
        }

        @objc func doubleTapped(_ g: UITapGestureRecognizer) {
            guard let sv = scroll, let v = tiled else { return }
            let p = g.location(in: v)
            let target = min(sv.zoomScale * 2, sv.maximumZoomScale)
            UIView.animate(withDuration: 0.3) {
                sv.zoomScale = target
                self.center(on: p)
            }
        }
    }
}

final class TiledMapView: UIView {
    override class var layerClass: AnyClass { CATiledLayer.self }
    var params: JSON = [:]
    var maxLevel = 0
    private static let cache = NSCache<NSString, UIImage>()

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        // device pixels per world pixel
        let scale = abs(ctx.ctm.a)
        let level = scale >= 1 ? 0 : min(maxLevel, max(0, Int(floor(log2(1 / scale)))))
        let chunk = CGFloat(512 << level)
        let x0 = Int(floor(rect.minX / chunk)), x1 = Int(floor((rect.maxX - 0.01) / chunk))
        let y0 = Int(floor(rect.minY / chunk)), y1 = Int(floor((rect.maxY - 0.01) / chunk))
        ctx.interpolationQuality = scale >= 1 ? .none : .medium
        guard x0 <= x1, y0 <= y1 else { return }
        for cy in y0...y1 {
            for cx in x0...x1 where cx >= 0 && cy >= 0 {
                guard let img = tile(level: level, cx: cx, cy: cy) else { continue }
                let r = CGRect(x: CGFloat(cx) * chunk, y: CGFloat(cy) * chunk, width: chunk, height: chunk)
                img.draw(in: r)
            }
        }
    }

    private func tile(level: Int, cx: Int, cy: Int) -> UIImage? {
        var p = params.object
        p["level"] = .number(Double(level)); p["cx"] = .number(Double(cx)); p["cy"] = .number(Double(cy))
        let key = "\(params.rev.int ?? 0)|\(params.scope.int ?? -1)|\(params.version.str)|\(params.night.bool)|\(level)|\(cx)|\(cy)" as NSString
        if let c = Self.cache.object(forKey: key) { return c }
        guard let data = Core.shared.pngSync("map.chunk", .object(p)), let img = UIImage(data: data) else { return nil }
        Self.cache.setObject(img, forKey: key, cost: data.count)
        return img
    }
}

/// Markers, the focus ring and the trip path, drawn in screen space.
final class MarkerView: UIView {
    var data = MapOverlayData()
    var transformInfo: (CGPoint, CGFloat) = (.zero, 1)

    private func screen(_ p: CGPoint) -> CGPoint {
        CGPoint(x: p.x * transformInfo.1 - transformInfo.0.x, y: p.y * transformInfo.1 - transformInfo.0.y)
    }

    static func color(_ kind: String) -> UIColor {
        switch kind {
        case "trainer": return UIColor(red: 1, green: 0.27, blue: 0.27, alpha: 1)
        case "item": return UIColor(red: 0.27, green: 0.85, blue: 0.27, alpha: 1)
        case "hidden": return UIColor(red: 1, green: 0.4, blue: 1, alpha: 1)
        case "berry": return UIColor(red: 0.4, green: 0.87, blue: 0.8, alpha: 1)
        case "sign": return .white
        case "warp": return UIColor(red: 1, green: 0.67, blue: 0, alpha: 1)
        case "npc": return UIColor(red: 0.53, green: 0.53, blue: 1, alpha: 1)
        default: return UIColor(red: 0.75, green: 0.59, blue: 0.38, alpha: 1)
        }
    }

    static func letter(_ kind: String) -> String {
        switch kind {
        case "trainer": return "T"
        case "item": return "!"
        case "hidden": return "?"
        case "berry": return "B"
        case "sign": return "S"
        case "warp": return "W"
        case "obstacle": return "X"
        default: return ""
        }
    }

    override func draw(_ rect: CGRect) {
        guard let ctx = UIGraphicsGetCurrentContext() else { return }
        let z = transformInfo.1
        let r: CGFloat = z < 0.2 ? 2.5 : (z < 0.35 ? 4 : (z < 0.6 ? 6 : 9))
        let bounds = self.bounds.insetBy(dx: -20, dy: -20)
        // trip path
        if data.trip.count > 1 {
            ctx.setStrokeColor(UIColor.systemOrange.withAlphaComponent(0.9).cgColor)
            ctx.setLineWidth(3)
            ctx.setLineDash(phase: 0, lengths: [8, 5])
            ctx.beginPath()
            for (i, (p, _)) in data.trip.enumerated() {
                let s = screen(p)
                if i == 0 { ctx.move(to: s) } else { ctx.addLine(to: s) }
            }
            ctx.strokePath()
            ctx.setLineDash(phase: 0, lengths: [])
        }
        for m in data.objects {
            let s = screen(m.point)
            guard bounds.contains(s) else { continue }
            // zoomed far out, only fights and pickups
            if z < 0.35 && !(m.kind == "trainer" || m.kind == "item") && m.index != data.selected { continue }
            let c = Self.color(m.kind)
            let circle = CGRect(x: s.x - r, y: s.y - r, width: r * 2, height: r * 2)
            ctx.setFillColor(c.withAlphaComponent(m.routed ? 0.35 : 0.95).cgColor)
            ctx.fillEllipse(in: circle)
            ctx.setStrokeColor(UIColor.black.withAlphaComponent(0.6).cgColor)
            ctx.setLineWidth(1)
            ctx.strokeEllipse(in: circle)
            if r >= 9 {
                let l = m.routed ? "✓" : Self.letter(m.kind)
                let attrs: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: 10, weight: .heavy), .foregroundColor: UIColor.black]
                let sz = (l as NSString).size(withAttributes: attrs)
                (l as NSString).draw(at: CGPoint(x: s.x - sz.width / 2, y: s.y - sz.height / 2), withAttributes: attrs)
            }
            if m.index == data.selected {
                ctx.setStrokeColor(UIColor.white.cgColor)
                ctx.setLineWidth(3)
                ctx.strokeEllipse(in: circle.insetBy(dx: -5, dy: -5))
            }
        }
        // numbered trip discs
        for (p, label) in data.trip {
            let s = screen(p)
            let attrs: [NSAttributedString.Key: Any] = [.font: UIFont.systemFont(ofSize: 11, weight: .bold), .foregroundColor: UIColor.white]
            let sz = (label as NSString).size(withAttributes: attrs)
            let w = max(sz.width + 10, 20)
            let disc = CGRect(x: s.x - w / 2, y: s.y - 10 - 18, width: w, height: 20)
            ctx.setFillColor(UIColor.systemOrange.cgColor)
            ctx.addPath(UIBezierPath(roundedRect: disc, cornerRadius: 10).cgPath)
            ctx.fillPath()
            (label as NSString).draw(at: CGPoint(x: disc.midX - sz.width / 2, y: disc.midY - sz.height / 2), withAttributes: attrs)
        }
        // focus
        if let fr = data.focusRect {
            let a = screen(fr.origin), b = screen(CGPoint(x: fr.maxX, y: fr.maxY))
            ctx.setStrokeColor(UIColor.systemYellow.cgColor)
            ctx.setLineWidth(3)
            ctx.stroke(CGRect(x: a.x, y: a.y, width: b.x - a.x, height: b.y - a.y))
        }
        for p in data.focus {
            let s = screen(p)
            ctx.setStrokeColor(UIColor.systemYellow.cgColor)
            ctx.setLineWidth(4)
            ctx.strokeEllipse(in: CGRect(x: s.x - 20, y: s.y - 20, width: 40, height: 40))
            ctx.setStrokeColor(UIColor.black.withAlphaComponent(0.5).cgColor)
            ctx.setLineWidth(1.5)
            ctx.strokeEllipse(in: CGRect(x: s.x - 23, y: s.y - 23, width: 46, height: 46))
        }
    }
}
