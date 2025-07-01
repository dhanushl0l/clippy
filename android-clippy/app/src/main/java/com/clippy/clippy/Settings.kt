package com.clippy.clippy

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Divider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.sp

@Composable
fun settings() {
    var text by remember { mutableStateOf("") }
    var text_ls by remember { mutableStateOf(listOf<String>()) }
    Column(
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier =
            Modifier
                .background(MaterialTheme.colorScheme.background)
                .fillMaxSize(),
    ) {
        val count = remember { mutableIntStateOf(0) }
        Text(
            text = count.intValue.toString(),
            color = Color.Red,
            fontSize = 30.sp,
            textAlign = TextAlign.Left,
        )

        TextField(
            value = text,
            onValueChange = { text = it },
            label = { Text("Settbitch") },
        )
        Button(onClick = {
            if (text.isNotBlank()) {
                text_ls = text_ls + text
                text = ""
            }
        }) {
            Text("add 1 {${count.intValue}}")
        }
        LazyColumn {
            items(text_ls) { current ->
                Column {
                    Text(text = current)
                    Divider()
                }
            }
        }
    }
}
