import Foundation

class API {
    private let baseURL = URL(string: Bundle.main.infoDictionary!["BASE_URL"] as! String)!

    enum TaskCreationParams: Codable {
        case url(paramType: String = "url", url: String)
        case raw(paramType: String = "raw", url: String, raw: String)

        enum Keys: CodingKey {
            case paramType, url, raw
        }

        func encode(to encoder: any Encoder) throws {
            var container = encoder.container(keyedBy: Keys.self)
            switch self {
            case let .url(paramType, url):
                try container.encode(paramType, forKey: .paramType)
                try container.encode(url, forKey: .url)
            case let .raw(paramType, url, raw):
                try container.encode(paramType, forKey: .paramType)
                try container.encode(url, forKey: .url)
                try container.encode(raw, forKey: .raw)
            }
        }
    }

    struct TaskCreationResult: Codable { let id: String }

    struct TaskQueryResult: Codable {
        let id: String
        let status: String
        let message: String
    }

    struct HTTPError: LocalizedError {
        let statusCode: Int

        var errorDescription: String {
            HTTPURLResponse.localizedString(forStatusCode: self.statusCode)
        }
    }

    private func url(_ path: String) -> URL { baseURL.appending(path: path) }

    private func fetch(_ req: URLRequest) async throws -> Data {
        let (body, res) = try await URLSession.shared.data(for: req)
        let status = (res as! HTTPURLResponse).statusCode
        guard (200...299).contains(status) else { throw HTTPError(statusCode: status) }
        return body
    }

    func createTask(with params: TaskCreationParams) async throws -> TaskCreationResult {
        var req = URLRequest(url: url("/api/download"))
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(params)
        return try JSONDecoder().decode(TaskCreationResult.self, from: await fetch(req))
    }

    func queryTask(by id: String) async throws -> TaskQueryResult {
        return try JSONDecoder().decode(
            TaskQueryResult.self, from: await fetch(URLRequest(url: url("/api/download/\(id)"))))
    }

    func downloadFile(by id: String) async throws -> Data {
        return try await fetch(URLRequest(url: url("/api/download/file/\(id)")))
    }
}
