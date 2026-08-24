package com.comeixalpha.colorify

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // 官方推荐：edge-to-edge 全面屏（内容延伸到状态栏/导航栏，自动适配刘海屏）
        // 必须在 super.onCreate 之前调用
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
    }
}