import Foundation
import Observation

/// Downloads a gen 4/5 game's map pictures from the desktop app's GitHub
/// releases (the same `imagery_<game>_<sha>.zip` assets the desktop
/// fetches), then hands the zip to the core, which checks its SHA-256.
@MainActor
@Observable
final class ImageryDownloader: NSObject {
    var progress: Double = 0
    var running = false
    var error: String?
    private var task: URLSessionDownloadTask?
    private var continuation: CheckedContinuation<URL, Error>?

    static let releases = "https://api.github.com/repos/Scotts-Thoughts/pkmn_yellow_xp_router/releases"

    func download(asset: String) async -> URL? {
        running = true
        error = nil
        progress = 0
        defer { running = false }
        do {
            let url = try await findAsset(asset)
            return try await fetch(url)
        } catch {
            if (error as NSError).code != NSURLErrorCancelled { self.error = error.localizedDescription }
            return nil
        }
    }

    func cancel() { task?.cancel() }

    private func findAsset(_ name: String) async throws -> URL {
        for page in 1...10 {
            var req = URLRequest(url: URL(string: "\(Self.releases)?per_page=100&page=\(page)")!)
            req.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
            req.setValue("xp-router-ios", forHTTPHeaderField: "User-Agent")
            let (data, _) = try await URLSession.shared.data(for: req)
            let releases = JSON.parse(data).array
            if releases.isEmpty { break }
            for r in releases where !r.draft.bool {
                for a in r.assets.array where a.name.str == name {
                    if let u = a.browser_download_url.string, let url = URL(string: u) { return url }
                }
            }
        }
        throw CoreError(message: "The map pictures (\(name)) aren't on any release.")
    }

    private func fetch(_ url: URL) async throws -> URL {
        let session = URLSession(configuration: .default, delegate: self, delegateQueue: .main)
        return try await withCheckedThrowingContinuation { cont in
            continuation = cont
            let t = session.downloadTask(with: url)
            task = t
            t.resume()
        }
    }
}

extension ImageryDownloader: URLSessionDownloadDelegate {
    nonisolated func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        // move it before the system deletes it
        let dest = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".zip")
        let moved = Result { try FileManager.default.moveItem(at: location, to: dest) }
        MainActor.assumeIsolated {
            switch moved {
            case .success: continuation?.resume(returning: dest)
            case .failure(let e): continuation?.resume(throwing: e)
            }
            continuation = nil
        }
    }

    nonisolated func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        let p = totalBytesExpectedToWrite > 0 ? Double(totalBytesWritten) / Double(totalBytesExpectedToWrite) : 0
        MainActor.assumeIsolated { progress = p }
    }

    nonisolated func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let error else { return }
        MainActor.assumeIsolated {
            continuation?.resume(throwing: error)
            continuation = nil
        }
    }
}
