package com.comeixalpha.colorify.settings

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@TauriPlugin
class SettingsPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun openAllFilesAccess(invoke: Invoke) {
        try {
            val intent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION)
            intent.data = Uri.parse("package:${activity.packageName}")
            activity.startActivity(intent)
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }

    @Command
    fun checkAllFilesAccess(invoke: Invoke) {
        try {
            val granted =
                Build.VERSION.SDK_INT >= Build.VERSION_CODES.R &&
                    Environment.isExternalStorageManager()
            invoke.resolveObject(granted)
        } catch (e: Exception) {
            invoke.reject(e.message)
        }
    }
}
