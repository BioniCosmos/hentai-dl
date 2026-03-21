@file:OptIn(ExperimentalMaterial3Api::class)

package net.witch.hentaidownloader

import android.annotation.SuppressLint
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import io.ktor.client.HttpClient
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.client.statement.bodyAsText
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import net.witch.hentaidownloader.ui.theme.HentaiDownloaderTheme
import kotlin.time.Duration.Companion.seconds

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            HentaiDownloaderTheme {
                val snackbarHostState = remember { SnackbarHostState() }
                Scaffold(
                    Modifier.fillMaxSize(),
                    { TopAppBar({ Text("Hentai Downloader") }) },
                    snackbarHost = { SnackbarHost(snackbarHostState) },
                ) { innerPadding ->
                    val navController = rememberNavController()
                    NavHost(navController, Main) {
                        val modifier = Modifier.padding(innerPadding)
                        composable<Main> {
                            MainScreen(modifier, snackbarHostState, navController)
                        }
                        composable<AuthWebView> { entry ->
                            val authWebView = entry.toRoute<AuthWebView>()
                            AuthWebViewScreen(authWebView.url, {
                                navController.previousBackStackEntry?.savedStateHandle?.set(
                                    "doc", it
                                )
                                navController.popBackStack()
                            }, modifier)
                        }
                    }
                }
            }
        }
    }
}

@Serializable
private object Main

@Serializable
private class AuthWebView(val url: String)

private enum class Tab { URL, File }

private enum class URL(val url: String) { Houhuayuan("houhuayuan.vip"), Telegraph("telegra.ph") }

@Composable
private fun MainScreen(
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

        val pickFile = rememberFilePicker()
        HorizontalPager(
            pager,
            Modifier
                .fillMaxSize()
                .padding(16.dp),
            verticalAlignment = Alignment.Top,
        ) { page ->
            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                var url by remember { mutableStateOf("") }
                when (Tab.entries[page]) {
                    Tab.URL -> {
                        TextField(
                            url,
                            { url = it },
                            Modifier.fillMaxWidth(),
                            label = { Text("URL") },
                        )
                    }

                    Tab.File -> {
                        var open by remember { mutableStateOf(false) }
                        var url by remember { mutableStateOf(URL.Houhuayuan) }
                        var file by remember {
                            mutableStateOf<Pair<String, String>?>(
                                null
                            )
                        }
                        ExposedDropdownMenuBox(open, { open = it }) {
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
                    }
                }
                Button({
                    if (url.contains(URL.Houhuayuan.url)) {
                        navController.navigate(AuthWebView(url))
                    }
                }, Modifier.fillMaxWidth()) { Text("Download") }
            }
        }

        val entry by navController.currentBackStackEntryAsState()
        entry?.also {
            val doc by remember(entry) {
                it.savedStateHandle.getStateFlow(
                    "doc", ""
                )
            }.collectAsStateWithLifecycle()
            LaunchedEffect(doc) {
                if (doc.isEmpty()) {
                    return@LaunchedEffect
                }
                snackbarHostState.showSnackbar(withContext(Dispatchers.IO) {
                    try {
                        HttpClient().post(BuildConfig.BASE_URL) { setBody(doc) }.bodyAsText()
                    } catch (e: Exception) {
                        e.printStackTrace()
                        e.message ?: "unknown error"
                    }
                })
            }
        }
    }
}

@Composable
private fun AuthWebViewScreen(
    url: String, onReady: (String) -> Unit, modifier: Modifier = Modifier
) {
    val scope = rememberCoroutineScope()
    AndroidView({
        @SuppressLint("SetJavaScriptEnabled") WebView(it).apply {
            webViewClient = WebViewClient()
            settings.javaScriptEnabled = true

            addJavascriptInterface(object {
                @JavascriptInterface
                @Suppress("unused")
                fun send(s: String) = scope.launch { onReady(s) }
            }, "bridge")

            loadUrl(url)
            scope.launch {
                while (title?.contains("蔷薇后花园") != true) {
                    delay(1.seconds)
                }
                evaluateJavascript("bridge.send(document.documentElement.outerHTML)", null)
            }
        }
    }, modifier.fillMaxSize())
}

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
