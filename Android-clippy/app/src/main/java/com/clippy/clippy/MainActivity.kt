package com.clippy.clippy

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import com.clippy.clippy.ui.theme.ClippyTheme
import androidx.compose.runtime.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Email
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material.icons.outlined.Notifications
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBarItem
import androidx.navigation.NavController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val navigation = rememberNavController()
            ClippyTheme {
                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                    bottomBar = { Navbar(navigation) }
                ) { innerPadding ->
                    NavHost(
                        navController = navigation,
                        startDestination = "Clipboard",
                        modifier = Modifier.padding(innerPadding)
                    ) {
                        composable("Notifications") {
                            Notification()
                        }
                        composable("Clipboard") {
                            Clipboard()
                        }
                        composable("Settings") {
                            Settings()
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun Navbar(navController: NavController) {
    val items = listOf("Clipboard", "Notifications", "Settings")
    val selectedIcons = listOf(Icons.Filled.Email, Icons.Filled.Notifications, Icons.Filled.Settings)
    val unselectedIcons = listOf(Icons.Outlined.Email, Icons.Outlined.Notifications, Icons.Outlined.Settings)

    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentRoute = navBackStackEntry?.destination?.route
    val selectedIndex = items.indexOf(currentRoute)

    NavigationBar {
        items.forEachIndexed { index, item ->
            NavigationBarItem(
                icon = {
                    Icon(
                        if (selectedIndex == index) selectedIcons[index] else unselectedIcons[index],
                        contentDescription = item
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
                }
            )
        }
    }
}
