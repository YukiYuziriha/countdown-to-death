package com.countdowntodeath.app

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.view.WindowInsets
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.Toast
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.IOException

class MainActivity : Activity() {
    private lateinit var web: WebView

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Reminders.ensureChannel(this)
        web = WebView(this).apply {
            setBackgroundColor(Color.rgb(12, 13, 19))
            settings.apply {
                javaScriptEnabled = true
                domStorageEnabled = true
                allowContentAccess = false
                allowFileAccess = false
                javaScriptCanOpenWindowsAutomatically = false
                setSupportMultipleWindows(false)
            }
            webViewClient = LocalPages()
            addJavascriptInterface(CountdownBridge(), BRIDGE)
        }
        val root = FrameLayout(this).apply {
            setBackgroundColor(Color.rgb(12, 13, 19))
            addView(web, FrameLayout.LayoutParams(-1, -1))
            if (Build.VERSION.SDK_INT >= 35) {
                setOnApplyWindowInsetsListener { view, insets ->
                    val safe = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout() or WindowInsets.Type.ime())
                    view.setPadding(safe.left, safe.top, safe.right, safe.bottom)
                    WindowInsets.CONSUMED
                }
            }
        }
        setContentView(root)
        web.loadUrl(ENTRY)
    }

    override fun onResume() {
        super.onResume()
        Reminders.refresh(this)
        publishStatus()
    }

    override fun onDestroy() {
        web.removeJavascriptInterface(BRIDGE)
        (web.parent as? FrameLayout)?.removeView(web)
        web.destroy()
        super.onDestroy()
    }

    private fun publishStatus() {
        web.evaluateJavascript("window.dispatchEvent(new Event('countdown-host-status-changed'));", null)
    }

    private fun openSettings(intent: Intent) {
        try {
            startActivity(intent)
        } catch (_: ActivityNotFoundException) {
            Toast.makeText(this, "Change reminder access from Android Settings.", Toast.LENGTH_LONG).show()
        }
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode == NOTIFICATION_REQUEST) publishStatus()
    }

    private inner class CountdownBridge {
        @JavascriptInterface
        fun getState(): String = Reminders.getState(applicationContext)

        @JavascriptInterface
        fun saveState(json: String) = Reminders.saveState(applicationContext, json)

        @JavascriptInterface
        fun getStatus(): String {
            val exact = Reminders.exactAlarmsEnabled(applicationContext)
            return JSONObject().put("notifications", Reminders.notificationsEnabled(applicationContext))
                .put("exactAlarms", exact)
                .put("statusText", if (exact) "Precise reminders enabled." else "Approximate reminders are scheduled; Android may deliver them late.")
                .toString()
        }

        @JavascriptInterface
        fun requestNotifications() = runOnUiThread {
            if (isDestroyed || Reminders.notificationsEnabled(this@MainActivity)) return@runOnUiThread
            val asked = getPreferences(MODE_PRIVATE)
            if (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED &&
                (!asked.getBoolean("asked_notifications", false) || shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS))) {
                asked.edit().putBoolean("asked_notifications", true).apply()
                requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), NOTIFICATION_REQUEST)
            } else {
                openSettings(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, packageName))
            }
        }

        @JavascriptInterface
        fun requestExactAlarms() = runOnUiThread {
            if (!isDestroyed && Build.VERSION.SDK_INT >= 31 && !Reminders.exactAlarmsEnabled(this@MainActivity)) {
                openSettings(Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, Uri.fromParts("package", packageName, null)))
            }
        }
    }

    private inner class LocalPages : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean =
            !request.isForMainFrame || !isEntry(request.url)

        override fun onPageStarted(view: WebView, url: String, favicon: android.graphics.Bitmap?) {
            if (!isEntry(Uri.parse(url))) {
                view.removeJavascriptInterface(BRIDGE)
                view.stopLoading()
            }
        }

        override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse {
            val url = request.url
            val path = url.encodedPath?.removePrefix("/").orEmpty()
            if (request.method != "GET" || !sameOrigin(url) || request.isForMainFrame != isEntry(url)) return blocked()
            val type = topLevelTypes[path] ?: path.takeIf { assetPath.matches(it) && ".." !in it }
                ?.substringAfterLast('.')?.let(assetTypes::get) ?: return blocked()
            return try {
                WebResourceResponse(type, if (type.startsWith("text/")) "utf-8" else null, 200, "OK", responseHeaders, assets.open(path))
            } catch (_: IOException) {
                blocked()
            }
        }
    }

    private companion object {
        const val BRIDGE = "CountdownAndroid"
        const val HOST = "countdown.invalid"
        const val ENTRY = "https://$HOST/ui-prototype.html"
        const val NOTIFICATION_REQUEST = 1

        val topLevelTypes = mapOf(
            "ui-prototype.html" to "text/html", "prototype.css" to "text/css",
            "animated-texture.js" to "text/javascript", "reminder-schedule.js" to "text/javascript",
            "countdown-format.js" to "text/javascript", "app-host.js" to "text/javascript",
        )
        val assetPath = Regex("assets/(?:[A-Za-z0-9_-]+/)*[A-Za-z0-9_.-]+")
        val assetTypes = mapOf("ttf" to "font/ttf", "woff" to "font/woff", "woff2" to "font/woff2",
            "otf" to "font/otf", "png" to "image/png", "webp" to "image/webp", "jpg" to "image/jpeg", "jpeg" to "image/jpeg")
        // The bridge is injected into all frames by WebView; the only served page must not create any frames.
        val responseHeaders = mapOf(
            "Content-Security-Policy" to "default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; " +
                "font-src 'self'; img-src 'self' data:; frame-src 'none'; child-src 'none'; worker-src 'none'; " +
                "frame-ancestors 'none'; base-uri 'none'; form-action 'none'",
            "X-Content-Type-Options" to "nosniff",
        )

        fun sameOrigin(uri: Uri) = uri.scheme == "https" && uri.encodedAuthority == HOST && uri.encodedQuery == null
        fun isEntry(uri: Uri) = sameOrigin(uri) && uri.encodedPath == "/ui-prototype.html"
        fun blocked() = WebResourceResponse("text/plain", "utf-8", 404, "Not Found", responseHeaders, ByteArrayInputStream(byteArrayOf()))
    }
}
