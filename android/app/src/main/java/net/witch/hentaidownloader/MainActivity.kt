@file:OptIn(ExperimentalMaterial3Api::class)

package net.witch.hentaidownloader

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.toRoute
import net.witch.hentaidownloader.ui.theme.HentaiDownloaderTheme

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
                    NavHost(navController, MainPage) {
                        val modifier = Modifier.padding(innerPadding)
                        composable<MainPage> {
                            MainPage(modifier, snackbarHostState, navController)
                        }
                        composable<AuthPage> { entry ->
                            val authPage = entry.toRoute<AuthPage>()
                            AuthPage(authPage.url, {
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
