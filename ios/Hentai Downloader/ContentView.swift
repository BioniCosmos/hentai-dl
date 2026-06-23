import Combine
import SwiftUI
import WebKit

struct ContentView: View {
    private static let defaultImportErr = "failed to import file"

    @State private var err = ""

    @State private var rawURL = ""
    @State private var showWebView = false

    @State private var site = Site.houhuayuan
    @State private var open = false

    @State private var downloadViewModel = DownloadViewModel()

    private var failed: Binding<Bool> {
        Binding(get: { !err.isEmpty }, set: { if !$0 { err = "" } })
    }

    private var url: URL? { URL(string: rawURL) }

    var body: some View {
        TabView {
            Tab("URL", systemImage: "network") {
                URLTab(downloadViewModel: downloadViewModel, err: $err)
            }
            Tab("File", systemImage: "folder") {
                Form {
                    Section {
                        Picker("URL", selection: $site) {
                            ForEach(Site.allCases) { url in Text(url.rawValue) }
                        }
                        Button("Pick file") { open = true }.fileImporter(
                            isPresented: $open, allowedContentTypes: [.html]
                        ) { result in
                            switch result {
                            case .success(let url):
                                guard url.startAccessingSecurityScopedResource() else {
                                    err = Self.defaultImportErr
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
                                } catch { err = error.localizedDescription }

                            case .failure(let err): self.err = err.localizedDescription
                            }
                        }
                    }
                    DownloadButton {}
                }
            }
        }.alert("Error", isPresented: failed) {
        } message: {
            Text(err)
        }.fileExporter(
            isPresented: $downloadViewModel.exporting,
            document: downloadViewModel.document,
            defaultFilename: downloadViewModel.name,
        ) {
            switch $0 {
            case .success(let url): print(url)
            case .failure(let err): self.err = err.localizedDescription
            }
            downloadViewModel.pending = false
        } onCancellation: {
            downloadViewModel.pending = false
        }
    }
}

private enum Site: String, CaseIterable, Identifiable {
    case houhuayuan = "houhuayuan.vip"
    case telegraph = "telegra.ph"

    var id: Self { self }
}

private struct URLTab: View {
    let downloadViewModel: DownloadViewModel
    @Binding var err: String

    @State private var rawURL = ""
    @State private var showWebView = false

    private var url: URL? { URL(string: rawURL) }

    var body: some View {
        NavigationStack {
            Form {
                Section { TextField("URL", text: $rawURL) }
                DownloadButton(downloadViewModel.pending) {
                    switch url {
                    case .some(let url) where url.host() ?? "" == Site.houhuayuan.rawValue:
                        showWebView = true
                    case .some:
                        downloadViewModel.download(with: .url(url: rawURL)) {
                            err = $0.errorDescription
                        }
                    case .none: err = "invalid URL"
                    }
                }
            }.navigationDestination(isPresented: $showWebView) {
                if let url = url {
                    WebView(url: url) {
                        showWebView = false
                        downloadViewModel.download(with: .raw(url: rawURL, raw: $0)) {
                            err = $0.errorDescription
                        }
                    }
                }
            }
        }
    }
}

private func DownloadButton(_ pending: Bool = false, action: @escaping () -> Void) -> some View {
    Button(action: action) {
        Text(!pending ? "Download" : "Downloading").frame(maxWidth: .infinity)
    }
    .buttonStyle(.borderedProminent)
    .controlSize(.large)
    .buttonBorderShape(.capsule)
    .listRowInsets(.init())
    .listRowBackground(Color.clear)
    .disabled(pending)
}

private struct WebView: UIViewRepresentable {
    typealias UIViewType = WKWebView

    let url: URL
    let onComplete: (String) -> Void

    func makeUIView(context: Context) -> UIViewType {
        let webView = WKWebView()
        webView.load(URLRequest(url: self.url))
        context.coordinator.crawler =
            webView
            .publisher(for: \.title)
            .first { $0?.contains("蔷薇后花园") ?? false }
            .sink { _ in
                webView.evaluateJavaScript("document.documentElement.outerHTML") { value, _ in
                    self.onComplete(value as! String)
                }
            }
        return webView
    }

    func updateUIView(_ uiView: UIViewType, context: Context) {}

    class Coordinator { var crawler: Cancellable? }

    func makeCoordinator() -> Coordinator { Coordinator() }
}

#Preview {
    ContentView()
}
