import SwiftUI
import UniformTypeIdentifiers

@MainActor @Observable
class DownloadViewModel {
    struct Error: LocalizedError { let errorDescription: String }

    private let api = API()

    var pending = false
    var exporting = false
    private(set) var name: String?
    private(set) var document: Downloaded?

    func download(with params: API.TaskCreationParams, onFailure: @escaping (Error) -> Void) {
        self.pending = true
        Task {
            @MainActor func fail(with e: String) {
                self.pending = false
                onFailure(.init(errorDescription: e))
            }

            do {
                let id = try await api.createTask(with: params).id

                while true {
                    let queryResult = try await api.queryTask(by: id)
                    let message = queryResult.message

                    switch queryResult.status {
                    case "pending":
                        try await Task.sleep(for: .seconds(1))
                        continue
                    case "done":
                        let data = try await api.downloadFile(by: id)
                        self.name = message
                        self.document = .init(data: data)
                        self.exporting = true
                    case "error": fail(with: message)
                    default: fatalError()
                    }
                    break
                }
            } catch { fail(with: error.localizedDescription) }
        }
    }
}

struct Downloaded: FileDocument {
    let data: Data

    static var readableContentTypes: [UTType] = [.text, .zip]

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        return FileWrapper(regularFileWithContents: data)
    }
}

extension Downloaded {
    init(configuration: ReadConfiguration) throws {
        fatalError("unimplemented")
    }
}
