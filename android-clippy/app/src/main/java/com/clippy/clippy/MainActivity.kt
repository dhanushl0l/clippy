package com.clippy.clippy

import android.Manifest
import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.annotation.RequiresApi
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.core.app.ActivityCompat
import androidx.navigation.NavController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.clippy.clippy.ui.theme.ClippyTheme

class MainActivity : ComponentActivity() {
    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        ActivityCompat.requestPermissions(
            this,
            arrayOf(Manifest.permission.POST_NOTIFICATIONS),
            0,
        )

        enableEdgeToEdge()

        if (!isServiceRunning(RunningServ::class.java)) {
            Intent(applicationContext, RunningServ::class.java).also {
                it.action = RunningServ.Service.START.toString()
                startService(it)
            }
        }

        setContent {
            val navigation = rememberNavController()
            ClippyTheme {
                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                    bottomBar = {
                        Navbar(navigation)
                    }
                ) { innerPadding ->
                    NavHost(
                        navController = navigation,
                        startDestination = "Clipboard",
                        modifier = Modifier.padding(innerPadding)
                    ) {
                        composable("Notifications") { notification() }
                        composable("Clipboard") { Clipboard(navigation) }
                        composable("Settings") { settings() }
                        composable("Search") { Search() }
                    }
                }
            }
        }
    }

    private fun isServiceRunning(serviceClass: Class<*>): Boolean {
        val manager = getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        @Suppress("DEPRECATION")
        for (service in manager.getRunningServices(Int.MAX_VALUE)) {
            if (serviceClass.name == service.service.className) return true
        }
        return false
    }
}

@Composable
fun Navbar(navController: NavController) {
    val items = listOf("Clipboard", "Notifications", "Settings")
    val selectedIcons = listOf(Icons.Filled.Email, Icons.Filled.Notifications, Icons.Filled.Settings)
    val unselectedIcons = listOf(Icons.Outlined.Email, Icons.Outlined.Notifications, Icons.Outlined.Settings)

    val navBackStackEntry = navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry.value?.destination?.route
    val selectedIndex = items.indexOf(currentRoute)

    NavigationBar {
        items.forEachIndexed { index, item ->
            NavigationBarItem(
                icon = {
                    Icon(
                        if (selectedIndex == index) selectedIcons[index] else unselectedIcons[index],
                        contentDescription = item,
                    )
                },
                label = { Text(item) },
                selected = selectedIndex == index,
                onClick = {
                    if (currentRoute != items[index]) {
                        navController.navigate(items[index]) {
                            launchSingleTop = true
                            popUpTo(navController.graph.startDestinationId) {
                                saveState = true
                            }
                            restoreState = true
                        }
                    }
                },
            )
        }
    }
}
