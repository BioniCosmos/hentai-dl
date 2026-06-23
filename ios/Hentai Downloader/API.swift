import Foundation

class API {
    private let baseURL = Bundle.main.infoDictionary!["BASE_URL"] as! String

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

    func createTask(with params: TaskCreationParams) async throws -> TaskCreationResult {
        var req = URLRequest(url: URL(string: "\(self.baseURL)/api/download")!)
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = try JSONEncoder().encode(params)

        return try JSONDecoder().decode(
            TaskCreationResult.self,
            from: await URLSession.shared.data(for: req).0,
        )
    }

    func queryTask(by id: String) async throws -> TaskQueryResult {
        return try JSONDecoder().decode(
            TaskQueryResult.self,
            from: await URLSession.shared.data(
                from: URL(string: "\(self.baseURL)/api/download/\(id)")!,
            ).0,
        )
    }

    func downloadFile(by id: String) async throws -> Data {
        return try await URLSession.shared.data(
            from: URL(string: "\(self.baseURL)/api/download/file/\(id)")!,
        ).0
    }
}
