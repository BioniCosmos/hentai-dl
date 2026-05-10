@file:OptIn(ExperimentalMaterial3Api::class)

package net.witch.hentaidownloader

import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.MenuAnchorType
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavController
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable

@Serializable
object MainPage

@Composable
fun MainPage(
    modifier: Modifier = Modifier,
    snackbarHostState: SnackbarHostState,
    navController: NavController,
) {
    Column(modifier) {
        val scope = rememberCoroutineScope()
        val pager = rememberPagerState { Tab.entries.size }

        PrimaryTabRow(pager.currentPage) {
            Tab.entries.forEachIndexed { i, tab ->
                Tab(
                    pager.currentPage == i,
                    { scope.launch { pager.animateScrollToPage(i) } },
                    text = { Text(tab.name) },
                )
            }
        }

        HorizontalPager(
            pager,
            Modifier
                .fillMaxSize()
                .padding(16.dp),
            verticalAlignment = Alignment.Top,
        ) { page ->
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                when (Tab.entries[page]) {
                    Tab.URL -> URLTab(snackbarHostState, navController, scope)
                    Tab.File -> FileTab(snackbarHostState, scope)
                }
            }
        }
    }
}

private enum class Tab { URL, File }

@Composable
private fun URLTab(
    snackbarHostState: SnackbarHostState,
    navController: NavController,
    scope: CoroutineScope,
    viewModel: MainViewModel = viewModel(),
) {
    var url by remember { mutableStateOf("") }

    TextField(
        url,
        { url = it },
        Modifier.fillMaxWidth(),
        label = { Text("URL") },
    )
    Button({
        viewModel.download(
            url,
            { scope.launch { snackbarHostState.showSnackbar("Done. Saved to $it") } },
            { scope.launch { snackbarHostState.showSnackbar("Error: ${it.message}") } },
        )
    }, Modifier.fillMaxWidth(), !viewModel.pending) { Text("Download") }
}

@Composable
private fun FileTab(snackbarHostState: SnackbarHostState, scope: CoroutineScope) {
    var open by remember { mutableStateOf(false) }
    var url by remember { mutableStateOf(URL.Houhuayuan) }
    var file by remember { mutableStateOf<Pair<String, String>?>(null) }
    val pickFile = rememberFilePicker()

    // CRAZY…
    @Suppress("AssignedValueIsNeverRead") ExposedDropdownMenuBox(open, { open = it }) {
        OutlinedTextField(
            url.url,
            {},
            modifier = Modifier
                .fillMaxWidth()
                .menuAnchor(MenuAnchorType.PrimaryNotEditable),
            readOnly = true,
            label = { Text("URL") },
        )
        ExposedDropdownMenu(open, { open = false }) {
            URL.entries.forEach {
                DropdownMenuItem({ Text(it.url) }, {
                    url = it
                    open = false
                })
            }
        }
    }
    Row(
        Modifier.fillMaxWidth(),
        Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Button({
            scope.launch {
                try {
                    file = pickFile()
                } catch (e: Exception) {
                    snackbarHostState.showSnackbar(
                        e.message ?: "unknown error"
                    )
                    e.printStackTrace()
                }
            }
        }) { Text("Pick file") }
        Text(file?.first ?: "")
    }
    Button({}, Modifier.fillMaxWidth()) { Text("Download") }
}

private enum class URL(val url: String) { Houhuayuan("houhuayuan.vip"), Telegraph("telegra.ph") }

@Composable
private fun rememberFilePicker(): suspend () -> Pair<String, String>? {
    var deferred: CompletableDeferred<Uri?>? = null
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) {
        deferred!!.complete(it)
    }
    val contentResolver = LocalContext.current.contentResolver
    return job@{
        deferred = CompletableDeferred()
        launcher.launch("text/html")
        val uri = deferred.await() ?: return@job null
        withContext(Dispatchers.IO) {
            Pair(
                contentResolver.query(uri, null, null, null, null)?.use {
                    if (!it.moveToFirst()) {
                        return@use null
                    }
                    val i = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (i == -1) {
                        return@use null
                    }
                    it.getString(i)
                } ?: uri.lastPathSegment ?: "unknown",
                (contentResolver.openInputStream(uri)
                    ?: throw Exception("failed to open the stream")).use { stream ->
                    stream.reader().use { it.readText() }
                },
            )
        }
    }
}
