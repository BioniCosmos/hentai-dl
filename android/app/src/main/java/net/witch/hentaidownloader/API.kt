package net.witch.hentaidownloader

import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.client.plugins.defaultRequest
import io.ktor.client.request.get
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsBytes
import io.ktor.http.ContentType
import io.ktor.http.contentType
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.Serializable

class API {
    private val client = HttpClient {
        install(ContentNegotiation) { json() }
        defaultRequest { url(BuildConfig.BASE_URL) }
        expectSuccess = true
    }

    sealed interface TaskCreationParams {
        @Serializable
        data class Url(val paramType: String = "url", val url: String) : TaskCreationParams

        @Serializable
        data class Raw(
            val paramType: String = "raw",
            val url: String,
            val raw: String,
        ) : TaskCreationParams
    }

    @Serializable
    data class TaskCreationResult(val id: String)

    @Serializable
    data class TaskQueryResult(val id: String, val status: String, val message: String)

    suspend fun createTask(params: TaskCreationParams) = this.client.post("/api/download") {
        contentType(ContentType.Application.Json)
        setBody(params)
    }.body<TaskCreationResult>()

    suspend fun queryTask(id: String) = this.client.get("/api/download/$id").body<TaskQueryResult>()

    suspend fun downloadFile(id: String) = this.client.get("/api/download/file/$id").bodyAsBytes()
}
