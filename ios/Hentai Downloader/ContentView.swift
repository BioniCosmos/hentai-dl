import SwiftUI

struct ContentView: View {
    enum URL: String, CaseIterable, Identifiable {
        case houhuayuan = "houhuayuan.vip"
        case telegraph = "telegra.ph"

        var id: Self { self }
    }

    private static let defaultImportErr = "failed to import file"

    @State private var url = ""

    @State private var site = URL.houhuayuan
    @State private var open = false
    @State private var importErr = ""

    private var importFailed: Binding<Bool> {
        Binding(get: { !importErr.isEmpty }, set: { if !$0 { importErr = "" } })
    }

    private var DownloadButton: some View {
        Button {
            print(url)
        } label: {
            Text("Download").frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.large)
        .buttonBorderShape(.capsule)
        .listRowInsets(.init())
        .listRowBackground(Color.clear)
    }

    var body: some View {
        TabView {
            Tab("URL", systemImage: "network") {
                Form {
                    Section { TextField("URL", text: $url) }
                    DownloadButton
                }
            }
            Tab("File", systemImage: "folder") {
                Form {
                    Section {
                        Picker("URL", selection: $site) {
                            ForEach(URL.allCases) { url in Text(url.rawValue) }
                        }
                        Button("Pick file") { open = true }.fileImporter(
                            isPresented: $open, allowedContentTypes: [.html]
                        ) { result in
                            switch result {
                            case .success(let url):
                                guard url.startAccessingSecurityScopedResource() else {
                                    importErr = Self.defaultImportErr
                                    return
                                }
                                defer { url.stopAccessingSecurityScopedResource() }
                                do {
                                    let content = try String(contentsOf: url, encoding: .utf8)
                                    print(
                                        """
                                        name=\(url.lastPathComponent)
                                        contentPrefix=\(content.prefix(100))
                                        """
                                    )
                                } catch { importErr = error.localizedDescription }
                            case .failure(let err): importErr = err.localizedDescription
                            }
                        }
                    }
                    DownloadButton
                }
            }
        }.alert("Error", isPresented: importFailed) {
        } message: {
            Text(importErr)
        }
    }
}

#Preview {
    ContentView()
}
