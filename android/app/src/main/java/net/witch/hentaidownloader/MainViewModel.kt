package net.witch.hentaidownloader

import android.app.Application
import android.content.ContentValues
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import androidx.annotation.UiThread
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.application
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import kotlin.coroutines.cancellation.CancellationException
import kotlin.time.Duration.Companion.seconds

class MainViewModel(application: Application) : AndroidViewModel(application) {
    private val api = API()

    var pending by mutableStateOf(false)
        private set

    fun download(
        params: API.TaskCreationParams,
        @UiThread onSuccess: (String) -> Unit,
        @UiThread onFailure: (Exception) -> Unit,
    ) {
        this.pending = true
        this.viewModelScope.launch(Dispatchers.IO) {
            val (id) = api.createTask(params)

            suspend fun fail(e: Exception) {
                withContext(Dispatchers.Main) {
                    pending = false
                    onFailure(e)
                }
            }

            while (true) {
                val (_, status, message) = api.queryTask(id)
                when (status) {
                    "pending" -> {
                        delay(1.seconds)
                        continue
                    }

                    "done" -> {
                        val resolver = application.contentResolver

                        val fileURI = resolver.insert(
                            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                                MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
                            } else {
                                MediaStore.Files.getContentUri("external")
                            },
                            ContentValues().apply {
                                put(MediaStore.Downloads.DISPLAY_NAME, message)
                                put(MediaStore.Downloads.IS_PENDING, 1)
                                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
                                    put(
                                        MediaStore.Downloads.DATA,
                                        File(
                                            Environment.getExternalStoragePublicDirectory(
                                                Environment.DIRECTORY_DOWNLOADS,
                                            ),
                                            message,
                                        ).absolutePath,
                                    )
                                }
                            },
                        )
                        if (fileURI == null) {
                            fail(Exception("failed to insert file to content provider"))
                            return@launch
                        }

                        try {
                            val stream = resolver.openOutputStream(fileURI)
                            if (stream == null) {
                                fail(Exception("failed to open stream"))
                                return@launch
                            }

                            stream.use { it.write(api.downloadFile(id)) }
                            resolver.update(
                                fileURI,
                                ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) },
                                null,
                                null,
                            )
                        } catch (e: Exception) {
                            // TODO: why should catch CancellationException?
                            if (e is CancellationException) {
                                throw e
                            }
                            e.printStackTrace()
                            fail(e)
                            return@launch
                        }

                        val cursor = resolver.query(
                            fileURI,
                            arrayOf(MediaStore.Downloads.DATA),
                            null,
                            null,
                            null,
                        )
                        if (cursor == null) {
                            fail(Exception("failed to get file path"))
                            return@launch
                        }

                        val path = cursor.use {
                            it.moveToFirst()
                            it.getString(it.getColumnIndexOrThrow(MediaStore.Downloads.DATA))
                        }
                        withContext(Dispatchers.Main) {
                            pending = false
                            onSuccess(path)
                        }
                    }

                    "error" -> fail(Exception(message))
                }
                break
            }
        }
    }
}
