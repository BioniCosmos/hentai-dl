package net.witch.hentaidownloader

import android.annotation.SuppressLint
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlin.time.Duration.Companion.seconds

@Serializable
class AuthPage(val url: String)

@Composable
fun AuthPage(url: String, onReady: (String) -> Unit, modifier: Modifier = Modifier) {
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
