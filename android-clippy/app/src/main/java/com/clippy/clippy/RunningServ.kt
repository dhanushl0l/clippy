package com.clippy.clippy

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.ClipboardManager
import android.content.Intent
import android.os.IBinder
import androidx.core.app.NotificationCompat

class RunningServ : Service() {
    private lateinit var clipboard: ClipboardManager
    private var clipboardText: String = "Listening to clipboard..."

    override fun onCreate() {
        super.onCreate()
        clipboard = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager

        // Listen to clipboard changes
        clipboard.addPrimaryClipChangedListener {
            val clip = clipboard.primaryClip
            if (clip != null && clip.itemCount > 0) {
                clipboardText = clip.getItemAt(0).coerceToText(this).toString()
                updateNotification()
            }
        }

        startForegroundService()
    }

    private fun startForegroundService() {
        val channelId = "running_channel"
        val channel =
            NotificationChannel(
                channelId,
                "Running Notification",
                NotificationManager.IMPORTANCE_LOW,
            )
        val manager = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(channel)

        val notification = buildNotification()
        startForeground(1, notification)
    }

    private fun updateNotification() {
        val notification = buildNotification()
        val manager = getSystemService(NOTIFICATION_SERVICE) as NotificationManager
        manager.notify(1, notification)
    }

    private fun buildNotification(): Notification =
        NotificationCompat
            .Builder(this, "running_channel")
            .setContentTitle("Clippy Running")
            .setContentText(clipboardText) // avoid too long
            .setSmallIcon(R.drawable.ic_launcher_foreground) // make sure it's valid
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()

    override fun onBind(intent: Intent?): IBinder? = null

    enum class Service {
        START,
        STOP,
    }
}
