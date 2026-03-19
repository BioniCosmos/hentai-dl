@file:OptIn(ExperimentalMaterial3Api::class)

package net.witch.hentaidownloader

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import net.witch.hentaidownloader.ui.theme.HentaiDownloaderTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            HentaiDownloaderTheme {
                Scaffold(
                    Modifier.fillMaxSize(),
                    { TopAppBar({ Text("Hentai Downloader") }) }) { innerPadding ->
                    Column(Modifier.padding(innerPadding)) {
                        val pager = rememberPagerState { Tab.entries.size }
                        val coroutineScope = rememberCoroutineScope()

                        PrimaryTabRow(pager.currentPage) {
                            Tab.entries.forEachIndexed { i, tab ->
                                Tab(
                                    pager.currentPage == i,
                                    { coroutineScope.launch { pager.animateScrollToPage(i) } },
                                    text = { Text(tab.name) })
                            }
                        }

                        HorizontalPager(
                            pager,
                            Modifier
                                .fillMaxSize()
                                .padding(16.dp),
                            verticalAlignment = Alignment.Top
                        ) { page ->
                            Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                when (Tab.entries[page]) {
                                    Tab.URL -> {
                                        var url by remember { mutableStateOf("") }
                                        TextField(
                                            url,
                                            { url = it },
                                            Modifier.fillMaxWidth(),
                                            label = { Text("URL") })
                                    }

                                    Tab.File -> {}
                                }
                                Button({}, Modifier.fillMaxWidth()) { Text("Download") }
                            }
                        }
                    }
                }
            }
        }
    }
}

private enum class Tab { URL, File }
