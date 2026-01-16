package ink.whis.floatingbubble

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.webkit.WebView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/**
 * Configuration for a specific bubble state.
 */
@InvokeArg
class StateConfig {
    /**
     * Icon resource name for this state (optional).
     * If not provided, uses the default icon.
     */
    var iconResourceName: String? = null
}

/**
 * Options for showing the floating bubble.
 */
@InvokeArg
class BubbleOptions {
    var size: Int = 60
    var startX: Int = 0
    var startY: Int = 100
    var iconResourceName: String? = null
    var background: String = "#1C1C1C"
    var states: Map<String, StateConfig>? = null
}

/**
 * Options for setting bubble state.
 */
@InvokeArg
class StateOptions {
    var state: String = "idle"
}

/**
 * Tauri plugin for displaying floating bubble overlays on Android.
 *
 * This plugin uses Android's WindowManager to show a draggable bubble
 * that persists across apps using the SYSTEM_ALERT_WINDOW permission.
 */
@TauriPlugin
class FloatingBubblePlugin(private val activity: Activity) : Plugin(activity) {

    companion object {
        private const val TAG = "FloatingBubblePlugin"
        private const val REQUEST_MICROPHONE_PERMISSION = 1001

        // Static flag to track bubble visibility across service restarts
        @Volatile
        var isBubbleVisible: Boolean = false

        // Track if the activity is in foreground (WebView is active)
        @Volatile
        var isActivityResumed: Boolean = false

        // Track if native recording is active (when app is backgrounded)
        @Volatile
        var isNativeRecording: Boolean = false

        // Reference to the plugin instance for sending events from the service
        @Volatile
        private var pluginInstance: FloatingBubblePlugin? = null

        // Reference to WebView for emitting events via JavaScript
        @Volatile
        var webViewInstance: WebView? = null

        /**
         * Emit a Tauri event via WebView JavaScript evaluation.
         */
        private fun emitTauriEvent(eventName: String, action: String) {
            val webView = webViewInstance
            if (webView == null) {
                Log.w(TAG, "emitTauriEvent($eventName): WebView is null")
                return
            }

            Log.d(TAG, "emitTauriEvent: Emitting $eventName")
            Handler(Looper.getMainLooper()).post {
                try {
                    val js = """
                        (function() {
                            console.log('[FloatingBubble] Emitting $eventName event');
                            if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {
                                window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
                                    event: '$eventName',
                                    payload: { action: '$action' }
                                }).catch(function(e) { console.error('Failed to emit event:', e); });
                            } else {
                                console.error('[FloatingBubble] TAURI_INTERNALS not available');
                            }
                        })();
                    """.trimIndent()
                    webView.evaluateJavascript(js, null)
                } catch (e: Exception) {
                    Log.e(TAG, "Error emitting $eventName event", e)
                }
            }
        }

        /**
         * Emit bubble click event to frontend.
         */
        fun invokeBubbleClick() {
            emitTauriEvent("floating-bubble://click", "click")
        }

        /**
         * Emit bubble close event to frontend.
         */
        fun invokeBubbleClose() {
            emitTauriEvent("floating-bubble://close", "close")
        }

        /**
         * Start native audio recording.
         * Called when bubble is clicked while app is backgrounded.
         */
        fun startNativeRecording() {
            Log.d(TAG, "startNativeRecording called")
            FloatingBubbleService.startRecording()
        }

        /**
         * Stop native audio recording.
         */
        fun stopNativeRecording() {
            Log.d(TAG, "stopNativeRecording called")
            FloatingBubbleService.stopRecording()
        }

        /**
         * Sync native recording state to frontend when app resumes.
         */
        fun syncNativeRecordingState() {
            if (!isNativeRecording) return

            val webView = webViewInstance ?: return
            Log.d(TAG, "Syncing native recording state to frontend")

            Handler(Looper.getMainLooper()).post {
                try {
                    val js = """
                        (function() {
                            console.log('[FloatingBubble] Syncing native recording state');
                            window.dispatchEvent(new CustomEvent('native-recording-active', {
                                detail: { isRecording: true }
                            }));
                        })();
                    """.trimIndent()
                    webView.evaluateJavascript(js, null)
                } catch (e: Exception) {
                    Log.e(TAG, "Error syncing native recording state", e)
                }
            }
        }
    }
    
    /**
     * JavaScript bridge for recording callbacks.
     * Allows JS to call native methods when async operations complete.
     */
    private inner class RecordingBridge {
        @android.webkit.JavascriptInterface
        fun onBackendReady() {
            Log.d(TAG, "RecordingBridge: Backend ready callback received")
            FloatingBubbleService.onBackendReady()
        }

        @android.webkit.JavascriptInterface
        fun onChunksFlushed() {
            Log.d(TAG, "RecordingBridge: Chunks flushed callback received")
            FloatingBubbleService.onChunksFlushed()
        }
    }

    override fun load(webView: WebView) {
        super.load(webView)
        pluginInstance = this
        webViewInstance = webView
        // Register JavaScript interface for recording callbacks
        webView.addJavascriptInterface(RecordingBridge(), "WhisRecordingBridge")
    }

    override fun onResume() {
        super.onResume()
        isActivityResumed = true
        Log.d(TAG, "Activity resumed - WebView active")

        // Sync native recording state to frontend if active
        syncNativeRecordingState()
    }

    override fun onPause() {
        super.onPause()
        isActivityResumed = false
        Log.d(TAG, "Activity paused - WebView inactive")
    }

    /**
     * Show the floating bubble with the given options.
     */
    @Command
    fun showBubble(invoke: Invoke) {
        val args = invoke.parseArgs(BubbleOptions::class.java)

        // Check if we have overlay permission
        if (!hasOverlayPermissionInternal()) {
            invoke.reject("Overlay permission not granted. Call requestOverlayPermission first.")
            return
        }

        try {
            Log.d(TAG, "showBubble called with args - size: ${args.size}, startX: ${args.startX}, startY: ${args.startY}")
            Log.d(TAG, "showBubble - defaultIcon: ${args.iconResourceName}, background: ${args.background}")
            Log.d(TAG, "showBubble - states: ${args.states}")

            // Pass configuration to the service via companion object
            FloatingBubbleService.bubbleSize = args.size
            FloatingBubbleService.bubbleStartX = args.startX
            FloatingBubbleService.bubbleStartY = args.startY
            FloatingBubbleService.defaultIconResourceName = args.iconResourceName
            FloatingBubbleService.backgroundColor = Color.parseColor(args.background)
            FloatingBubbleService.stateConfigs = args.states ?: emptyMap()

            Log.d(TAG, "showBubble - stateConfigs set to service: ${FloatingBubbleService.stateConfigs.size} states")

            // Start the floating bubble service
            val intent = Intent(activity, FloatingBubbleService::class.java)
            ContextCompat.startForegroundService(activity, intent)

            isBubbleVisible = true
            invoke.resolve()
        } catch (e: Exception) {
            Log.e(TAG, "showBubble failed: ${e.message}", e)
            invoke.reject("Failed to start bubble service: ${e.message}")
        }
    }
    
    /**
     * Parse a color string to an Int, with fallback.
     */
    private fun parseColor(color: String?, default: String): Int {
        return try {
            Color.parseColor(color ?: default)
        } catch (e: Exception) {
            Color.parseColor(default)
        }
    }

    /**
     * Hide the floating bubble.
     */
    @Command
    fun hideBubble(invoke: Invoke) {
        try {
            val intent = Intent(activity, FloatingBubbleService::class.java)
            activity.stopService(intent)
            isBubbleVisible = false
            FloatingBubbleService.resetState()
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Failed to stop bubble service: ${e.message}")
        }
    }

    /**
     * Check if the bubble is currently visible.
     */
    @Command
    fun isBubbleVisible(invoke: Invoke) {
        val result = JSObject()
        result.put("visible", isBubbleVisible)
        invoke.resolve(result)
    }

    /**
     * Request the SYSTEM_ALERT_WINDOW permission.
     * Opens system settings if permission is not granted.
     */
    @Command
    fun requestOverlayPermission(invoke: Invoke) {
        if (hasOverlayPermissionInternal()) {
            val result = JSObject()
            result.put("granted", true)
            invoke.resolve(result)
            return
        }

        try {
            val intent = Intent(
                Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                Uri.parse("package:${activity.packageName}")
            )
            activity.startActivity(intent)

            // Note: We can't wait for the result here, so we return false
            // The user should call hasOverlayPermission after returning to the app
            val result = JSObject()
            result.put("granted", false)
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject("Failed to open overlay permission settings: ${e.message}")
        }
    }

    /**
     * Check if the SYSTEM_ALERT_WINDOW permission is granted.
     */
    @Command
    fun hasOverlayPermission(invoke: Invoke) {
        val result = JSObject()
        result.put("granted", hasOverlayPermissionInternal())
        invoke.resolve(result)
    }

    /**
     * Check if the RECORD_AUDIO permission is granted.
     * This is required for foreground service with microphone type on Android 14+.
     */
    @Command
    fun hasMicrophonePermission(invoke: Invoke) {
        val result = JSObject()
        result.put("granted", hasMicrophonePermissionInternal())
        invoke.resolve(result)
    }

    /**
     * Request the RECORD_AUDIO permission.
     * Returns immediately - check hasMicrophonePermission after user responds.
     */
    @Command
    fun requestMicrophonePermission(invoke: Invoke) {
        if (hasMicrophonePermissionInternal()) {
            val result = JSObject()
            result.put("granted", true)
            invoke.resolve(result)
            return
        }

        try {
            ActivityCompat.requestPermissions(
                activity,
                arrayOf(Manifest.permission.RECORD_AUDIO),
                REQUEST_MICROPHONE_PERMISSION
            )

            // Note: We can't wait for the result here, so we return false
            // The user should call hasMicrophonePermission after returning to the app
            val result = JSObject()
            result.put("granted", false)
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject("Failed to request microphone permission: ${e.message}")
        }
    }

    /**
     * Update the bubble's visual state.
     */
    @Command
    fun setBubbleState(invoke: Invoke) {
        val args = invoke.parseArgs(StateOptions::class.java)

        try {
            Log.d(TAG, "setBubbleState command received with state: '${args.state}'")
            FloatingBubbleService.setState(args.state)
            Log.d(TAG, "setBubbleState command completed")
            invoke.resolve()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to update bubble state: ${e.message}", e)
            invoke.reject("Failed to update bubble state: ${e.message}")
        }
    }

    /**
     * Handle bubble click - emits event via WebView to notify the frontend.
     */
    @Command
    fun handleBubbleClick(invoke: Invoke) {
        Log.d(TAG, "handleBubbleClick command received")
        invokeBubbleClick()
        invoke.resolve()
    }

    /**
     * Handle bubble close - emits event via WebView to notify the frontend.
     */
    @Command
    fun handleBubbleClose(invoke: Invoke) {
        Log.d(TAG, "handleBubbleClose command received")
        invokeBubbleClose()
        invoke.resolve()
    }

    /**
     * Internal helper to check overlay permission.
     */
    private fun hasOverlayPermissionInternal(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            Settings.canDrawOverlays(activity)
        } else {
            true // Permission not required on older versions
        }
    }

    /**
     * Internal helper to check microphone permission.
     */
    private fun hasMicrophonePermissionInternal(): Boolean {
        return ContextCompat.checkSelfPermission(
            activity,
            Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED
    }
}
