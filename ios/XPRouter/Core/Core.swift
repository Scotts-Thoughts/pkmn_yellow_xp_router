import Foundation
import XPRCoreFFI

struct CoreError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// The Rust core (rust/crates/xpr-mobile). Every call runs on one serial
/// queue: the session is single-threaded, and the heavy work (route
/// recalculation, damage ranges, map rendering) stays off the main thread.
final class Core: @unchecked Sendable {
    static let shared = Core()
    private let queue = DispatchQueue(label: "xpr.core", qos: .userInitiated)
    private let imageQueue = DispatchQueue(label: "xpr.core.images", qos: .utility)

    let documents: URL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    let support: URL = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("pkmn_xp_router")

    private func decode(_ ptr: UnsafeMutablePointer<CChar>?) -> Result<JSON, CoreError> {
        guard let ptr else { return .failure(CoreError(message: "no response")) }
        let data = Data(bytes: ptr, count: strlen(ptr))
        xpr_free_string(ptr)
        let v = JSON.parse(data)
        if let err = v["err"].string { return .failure(CoreError(message: err)) }
        return .success(v["ok"])
    }

    func start() async throws {
        let cfg = support.path
        let data = documents.path
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            queue.async {
                let r = self.decode(xpr_init(cfg, data))
                switch r {
                case .success: cont.resume()
                case .failure(let e): cont.resume(throwing: e)
                }
            }
        }
    }

    private func encode(_ params: JSON) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: params.any, options: [.fragmentsAllowed]) else { return "{}" }
        return String(data: data, encoding: .utf8) ?? "{}"
    }

    @discardableResult
    func call(_ method: String, _ params: JSON = [:]) async throws -> JSON {
        let p = encode(params)
        return try await withCheckedThrowingContinuation { cont in
            queue.async {
                cont.resume(with: self.decode(xpr_call(method, p)).mapError { $0 as Error })
            }
        }
    }

    /// PNG bytes, blocking: for CATiledLayer's background drawing threads
    /// (never call it on the main thread).
    func pngSync(_ method: String, _ params: JSON) -> Data? {
        let p = encode(params)
        return queue.sync {
            var out: UnsafeMutablePointer<UInt8>? = nil
            let n = xpr_png(method, p, &out)
            guard n > 0, let out else { return nil }
            let data = Data(bytes: out, count: n)
            xpr_free_bytes(out, n)
            return data
        }
    }

    /// PNG bytes from the core (map chunks, sprites). Runs on the core
    /// queue too (the session owns the map caches).
    func png(_ method: String, _ params: JSON) async -> Data? {
        let p = encode(params)
        return await withCheckedContinuation { cont in
            queue.async {
                var out: UnsafeMutablePointer<UInt8>? = nil
                let n = xpr_png(method, p, &out)
                guard n > 0, let out else { cont.resume(returning: nil); return }
                let data = Data(bytes: out, count: n)
                xpr_free_bytes(out, n)
                cont.resume(returning: data)
            }
        }
    }
}
