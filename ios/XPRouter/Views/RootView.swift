import SwiftUI

struct RootView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        @Bindable var model = model
        Group {
            if model.started {
                NavigationStack(path: $model.path) {
                    LibraryView()
                        .navigationDestination(for: Screen.self) { screen in
                            switch screen {
                            case .editor: RouteEditorView()
                            case .event(let id): EventDetailView(eventID: id)
                            case .compare(let seed): CompareView(seed: seed)
                            case .map(let req): MapScreen(request: req)
                            case .runSummary: RunSummaryView()
                            case .settings: SettingsView()
                            case .customGens: CustomGensView()
                            }
                        }
                }
            } else if let err = model.startError {
                ContentUnavailableView("Couldn't start", systemImage: "exclamationmark.triangle", description: Text(err))
            } else {
                SplashView()
            }
        }
        .overlay(alignment: .top) {
            if let t = model.toast {
                ToastView(toast: t)
                    .padding(.top, 6)
                    .transition(.move(edge: .top).combined(with: .opacity))
                    .onTapGesture { model.toast = nil }
            }
        }
        .alert("Something went wrong", isPresented: Binding(get: { model.alert != nil }, set: { if !$0 { model.alert = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(model.alert ?? "")
        }
    }
}

struct SplashView: View {
    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "point.topleft.down.to.point.bottomright.curvepath.fill")
                .font(.system(size: 54, weight: .bold))
                .foregroundStyle(.tint)
                .symbolEffect(.pulse)
            Text("XP Router").font(.title2.weight(.bold))
            ProgressView()
        }
    }
}

struct ToastView: View {
    let toast: Toast
    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: toast.kind == "saved" ? "checkmark.circle.fill" : "info.circle.fill")
                .foregroundStyle(toast.kind == "saved" ? Theme.success : Color.accentColor)
            Text(toast.text).font(.subheadline.weight(.medium)).lineLimit(2)
        }
        .padding(.horizontal, 16).padding(.vertical, 11)
        .background(.regularMaterial, in: Capsule())
        .shadow(color: .black.opacity(0.15), radius: 12, y: 4)
        .padding(.horizontal)
    }
}
