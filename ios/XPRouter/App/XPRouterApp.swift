import SwiftUI

@main
struct XPRouterApp: App {
    @State private var model = AppModel()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .task { await model.start() }
                .onChange(of: phase) { _, p in
                    // iOS may end a backgrounded app: keep named routes saved
                    if p == .background, model.isOpen, model.app.unsaved.bool, !model.routeName.isEmpty {
                        Task { await model.mutate("route.save", ["name": .string(model.routeName)]) }
                    }
                }
                .onOpenURL { url in
                    // a route JSON shared from Files / Mail / AirDrop
                    Task {
                        let access = url.startAccessingSecurityScopedResource()
                        defer { if access { url.stopAccessingSecurityScopedResource() } }
                        if let r = await model.run("route.import", ["path": .string(url.path)]) {
                            model.show("Imported \(r.name.str)", kind: "saved")
                            await model.openRoute(r.name.str)
                        }
                    }
                }
        }
    }
}
