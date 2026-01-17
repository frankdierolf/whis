package ink.whis.floatingbubble

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Intent
import android.graphics.Color
import android.graphics.PixelFormat
import android.graphics.drawable.GradientDrawable
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.ImageView
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

/**
 * Foreground service that manages the floating bubble overlay.
 *
 * Uses standard Android WindowManager to create a draggable floating bubble.
 * Visual states change based on configured icons for each state.
 * Supports drag-to-close with a close zone at the bottom center.
 */
class FloatingBubbleService : Service() {

    companion object {
        private const val TAG = "FloatingBubbleService"
        private const val CHANNEL_ID = "floating_bubble_channel"
        private const val NOTIFICATION_ID = 1001
        private const val CLOSE_ZONE_SIZE = 80
        private const val CLOSE_ZONE_MARGIN = 16

        // Audio recording parameters - must match frontend expectations
        private const val SAMPLE_RATE = 16000  // 16kHz to match audioStreamer.ts
        private const val CHUNK_SIZE = 4096    // ~256ms at 16kHz

        // Whis palette colors
        private const val COLOR_BG_WEAK = "#1C1C1C"
        private const val COLOR_BG_WEAK_ALPHA = "#CC1C1C1C"
        private const val COLOR_BORDER = "#3D3D3D"
        private const val COLOR_RECORDING = "#FF4444"
        private const val COLOR_RECORDING_ALPHA = "#40FF4444"

        // Configuration passed from the plugin
        var bubbleSize: Int = 60
        var bubbleStartX: Int = 0
        var bubbleStartY: Int = 100
        var defaultIconResourceName: String? = null
        var backgroundColor: Int = Color.parseColor("#1C1C1C")
        var stateConfigs: Map<String, StateConfig> = emptyMap()
        var notificationConfig: NotificationConfig? = null

        // Reference to the current service instance for state updates
        @Volatile
        private var instance: FloatingBubbleService? = null

        // Store pending state when service isn't ready yet
        @Volatile
        private var pendingState: String? = null

        private val mainHandler = Handler(Looper.getMainLooper())

        /**
         * Run an action on the main thread with the current service instance.
         * Returns false if service is unavailable.
         */
        private inline fun withServiceOnMain(
            logTag: String? = null,
            crossinline action: FloatingBubbleService.() -> Unit
        ): Boolean {
            val service = instance
            if (service == null) {
                logTag?.let { Log.w(TAG, "$it: Service not available") }
                return false
            }
            mainHandler.post { service.action() }
            return true
        }

        /**
         * Update the bubble's state from outside the service.
         * If service isn't ready, stores the state for later application.
         */
        fun setState(state: String) {
            if (!withServiceOnMain { updateState(state) }) {
                pendingState = state
            }
        }

        /**
         * Reset static state when service is fully destroyed.
         */
        fun resetState() {
            pendingState = null
        }

        /**
         * Start native audio capture.
         */
        fun startCapture() {
            withServiceOnMain("startCapture") { startNativeCapture() }
        }

        /**
         * Stop native audio capture.
         */
        fun stopCapture() {
            withServiceOnMain("stopCapture") { stopNativeCapture() }
        }

        /**
         * Called from JavascriptInterface when consumer is ready for data.
         */
        fun onReady() {
            withServiceOnMain { executePendingCaptureCallback() }
        }

        /**
         * Called from JavascriptInterface when all data has been flushed.
         */
        fun onFlushed() {
            withServiceOnMain { executePendingStopCallback() }
        }
    }

    private var windowManager: WindowManager? = null
    private var bubbleView: ImageView? = null
    private var bubbleBackground: GradientDrawable? = null
    private var layoutParams: WindowManager.LayoutParams? = null
    private var closeZoneParams: WindowManager.LayoutParams? = null
    private var closeZoneView: FrameLayout? = null
    private var closeZoneIcon: ImageView? = null
    private var closeZoneBackground: GradientDrawable? = null
    private var currentStateName: String = "idle"
    private var closeZoneVisible = false
    private var closeZoneActivated = false

    // Native audio capture
    private var audioRecord: AudioRecord? = null
    private var captureThread: Thread? = null
    @Volatile
    private var isCapturing = false

    // Pending callback to start capture thread after consumer is ready
    @Volatile
    private var pendingCaptureCallback: (() -> Unit)? = null

    // Pending callback for when data is flushed before stopping
    @Volatile
    private var pendingStopCallback: (() -> Unit)? = null

    // Timeout handler for ready callback
    private val readyTimeoutHandler = Handler(Looper.getMainLooper())
    private val readyTimeoutRunnable = Runnable {
        Log.w(TAG, "Ready timeout - starting capture thread anyway")
        executePendingCaptureCallback()
    }

    // Timeout handler for flush callback
    private val flushTimeoutRunnable = Runnable {
        Log.w(TAG, "Flush timeout - stopping capture anyway")
        executePendingStopCallback()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        instance = this

        createNotificationChannel()
        startForeground(NOTIFICATION_ID, createNotification())

        windowManager = getSystemService(WINDOW_SERVICE) as WindowManager
        createCloseZone()
        createBubble()
    }

    override fun onDestroy() {
        super.onDestroy()

        // Clean up timeout handlers to prevent memory leaks
        readyTimeoutHandler.removeCallbacks(readyTimeoutRunnable)
        readyTimeoutHandler.removeCallbacks(flushTimeoutRunnable)

        // Stop native capture if active
        if (isCapturing) {
            Log.d(TAG, "Service destroying - stopping native capture")
            isCapturing = false
            try {
                captureThread?.join(500)
            } catch (e: InterruptedException) {
                // Ignore
            }
            captureThread = null
            audioRecord?.stop()
            audioRecord?.release()
            audioRecord = null
            FloatingBubblePlugin.isNativeCapture = false
        }

        instance = null
        removeCloseZone()
        removeBubble()
        FloatingBubblePlugin.isBubbleVisible = false
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Floating Bubble",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Voice input bubble service"
                setShowBadge(false)
            }
            val notificationManager = getSystemService(NotificationManager::class.java)
            notificationManager.createNotificationChannel(channel)
        }
    }

    private fun createCloseZone() {
        val density = resources.displayMetrics.density
        val sizePx = (CLOSE_ZONE_SIZE * density).toInt()
        val marginPx = (CLOSE_ZONE_MARGIN * density).toInt()

        closeZoneBackground = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(Color.parseColor(COLOR_BG_WEAK_ALPHA))
            setStroke((2 * density).toInt(), Color.parseColor(COLOR_BORDER))
        }

        closeZoneView = FrameLayout(this).apply {
            visibility = View.GONE
            this.background = closeZoneBackground

            // Use custom Whis-branded close icon
            val closeIconResId = resources.getIdentifier(
                "ic_close_zone",
                "drawable",
                packageName
            )

            closeZoneIcon = ImageView(this@FloatingBubbleService).apply {
                val drawable = if (closeIconResId != 0) {
                    ContextCompat.getDrawable(this@FloatingBubbleService, closeIconResId)
                } else {
                    null
                }
                if (drawable != null) {
                    setImageDrawable(drawable)
                } else {
                    setImageResource(android.R.drawable.ic_menu_close_clear_cancel)
                }
                setColorFilter(Color.WHITE)
                val padding = (sizePx * 0.25).toInt()
                setPadding(padding, padding, padding, padding)
            }

            addView(closeZoneIcon, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            ))
        }

        @Suppress("DEPRECATION")
        val windowType = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            WindowManager.LayoutParams.TYPE_PHONE
        }

        closeZoneParams = WindowManager.LayoutParams(
            sizePx,
            sizePx,
            windowType,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.BOTTOM or Gravity.CENTER_HORIZONTAL
            y = marginPx + sizePx
        }

        windowManager?.addView(closeZoneView, closeZoneParams)
    }

    private fun removeCloseZone() {
        closeZoneView?.let {
            try {
                windowManager?.removeView(it)
            } catch (e: Exception) {
                Log.e(TAG, "Error removing close zone view", e)
            }
        }
        closeZoneView = null
    }

    private fun showCloseZone() {
        if (closeZoneVisible) return
        closeZoneVisible = true
        closeZoneView?.visibility = View.VISIBLE
    }

    private fun hideCloseZone() {
        if (!closeZoneVisible) return
        closeZoneVisible = false
        closeZoneActivated = false
        closeZoneView?.visibility = View.GONE
        closeZoneBackground?.setColor(Color.parseColor(COLOR_BG_WEAK_ALPHA))
        closeZoneBackground?.setStroke((2 * resources.displayMetrics.density).toInt(), Color.parseColor(COLOR_BORDER))
    }

    private fun updateCloseZoneFeedback(isClose: Boolean) {
        if (isClose == closeZoneActivated) return
        closeZoneActivated = isClose
        val density = resources.displayMetrics.density
        if (isClose) {
            closeZoneBackground?.setColor(Color.parseColor(COLOR_RECORDING_ALPHA))
            closeZoneBackground?.setStroke((3 * density).toInt(), Color.parseColor(COLOR_RECORDING))
            closeZoneIcon?.setColorFilter(Color.parseColor(COLOR_RECORDING))
        } else {
            closeZoneBackground?.setColor(Color.parseColor(COLOR_BG_WEAK_ALPHA))
            closeZoneBackground?.setStroke((2 * density).toInt(), Color.parseColor(COLOR_BORDER))
            closeZoneIcon?.setColorFilter(Color.WHITE)
        }
    }

    private fun createBubble() {
        val density = resources.displayMetrics.density
        val sizePx = (Companion.bubbleSize * density).toInt()
        val currentBackgroundColor = Companion.backgroundColor
        val currentIconResourceName = Companion.defaultIconResourceName

        // Create circular background with configured color
        bubbleBackground = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(currentBackgroundColor)
        }

        // Create bubble view with default icon
        bubbleView = ImageView(this).apply {
            background = bubbleBackground

            // Load icon by resource name, fallback to default
            val iconResId = if (!currentIconResourceName.isNullOrEmpty()) {
                resources.getIdentifier(
                    currentIconResourceName,
                    "drawable",
                    packageName
                )
            } else {
                0
            }

            if (iconResId != 0) {
                try {
                    val iconDrawable = ContextCompat.getDrawable(
                        this@FloatingBubbleService,
                        iconResId
                    )
                    setImageDrawable(iconDrawable)
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to load icon: $currentIconResourceName", e)
                    loadDefaultIcon()
                }
            } else {
                // Try plugin's default icon, then fallback to system icon
                loadDefaultIcon()
            }

            scaleType = ImageView.ScaleType.CENTER_INSIDE
            val padding = (sizePx * 0.22).toInt()
            setPadding(padding, padding, padding, padding)

            contentDescription = "Floating bubble"
        }

        // Window layout params for overlay
        @Suppress("DEPRECATION")
        val windowType = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
        } else {
            WindowManager.LayoutParams.TYPE_PHONE
        }

        layoutParams = WindowManager.LayoutParams(
            sizePx,
            sizePx,
            windowType,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or
                WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.START
            x = (Companion.bubbleStartX * density).toInt()
            y = (Companion.bubbleStartY * density).toInt()
        }

        // Add touch listener for dragging
        bubbleView?.setOnTouchListener(BubbleTouchListener())

        // Add to window
        windowManager?.addView(bubbleView, layoutParams)
        FloatingBubblePlugin.isBubbleVisible = true

        // Apply any pending state that was set before service was ready
        val pending = pendingState
        if (pending != null) {
            pendingState = null
            updateState(pending)
        } else {
            currentStateName = "idle"
        }
    }

    /**
     * Load the plugin's default icon or fallback to system icon.
     */
    private fun ImageView.loadDefaultIcon() {
        // Try plugin's default icon first
        val defaultResId = resources.getIdentifier(
            "ic_floating_bubble_default",
            "drawable",
            packageName
        )

        if (defaultResId != 0) {
            try {
                val defaultDrawable = ContextCompat.getDrawable(
                    this@FloatingBubbleService,
                    defaultResId
                )
                setImageDrawable(defaultDrawable)
                return
            } catch (e: Exception) {
                // Fall through to system icon
            }
        }

        // Fallback to system icon
        setImageResource(android.R.drawable.ic_btn_speak_now)
    }

    private fun removeBubble() {
        bubbleView?.let {
            try {
                windowManager?.removeView(it)
            } catch (e: Exception) {
                Log.e(TAG, "Error removing bubble view", e)
            }
        }
        bubbleView = null
    }

    /**
     * Touch listener that handles dragging the bubble.
     *
     * Uses a long-press pattern to distinguish taps from drags:
     * - Quick tap: Toggle recording (click)
     * - Long press (400ms+): Enter drag mode, show close zone
     *
     * This prevents accidental drag activation on touch screens.
     */
    private inner class BubbleTouchListener : View.OnTouchListener {

        private var initialX = 0
        private var initialY = 0
        private var initialTouchX = 0f
        private var initialTouchY = 0f
        private var isDragging = false
        private var isDragEnabled = false  // Only true after long press
        private val clickThreshold = 10 // pixels
        private val longPressDelayMs = 400L // Time before drag mode activates

        private val handler = Handler(Looper.getMainLooper())
        private val longPressRunnable = Runnable {
            // Long press triggered - enable drag mode
            isDragEnabled = true
            showCloseZone()
            // Provide haptic feedback to indicate drag mode
            bubbleView?.performHapticFeedback(android.view.HapticFeedbackConstants.LONG_PRESS)
        }

        override fun onTouch(view: View, event: MotionEvent): Boolean {
            when (event.action) {
                MotionEvent.ACTION_DOWN -> {
                    initialX = layoutParams?.x ?: 0
                    initialY = layoutParams?.y ?: 0
                    initialTouchX = event.rawX
                    initialTouchY = event.rawY
                    isDragging = false
                    isDragEnabled = false
                    // Start long press timer - drag mode activates after delay
                    handler.postDelayed(longPressRunnable, longPressDelayMs)
                    return true
                }
                MotionEvent.ACTION_MOVE -> {
                    val deltaX = (event.rawX - initialTouchX).toInt()
                    val deltaY = (event.rawY - initialTouchY).toInt()

                    // Check if movement exceeds threshold
                    val hasMoved = kotlin.math.abs(deltaX) > clickThreshold ||
                        kotlin.math.abs(deltaY) > clickThreshold

                    if (hasMoved) {
                        if (isDragEnabled) {
                            // Long press already triggered - this is a drag
                            isDragging = true
                            layoutParams?.x = initialX + deltaX
                            layoutParams?.y = initialY + deltaY
                            windowManager?.updateViewLayout(bubbleView, layoutParams)
                            updateCloseZoneFeedback(isNearCloseZone())
                        } else {
                            // Movement before long press - cancel drag activation
                            // but allow small movements (finger jitter)
                            if (kotlin.math.abs(deltaX) > clickThreshold * 3 ||
                                kotlin.math.abs(deltaY) > clickThreshold * 3) {
                                handler.removeCallbacks(longPressRunnable)
                            }
                        }
                    }
                    return true
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    // Cancel long press timer if still pending
                    handler.removeCallbacks(longPressRunnable)
                    hideCloseZone()

                    if (!isDragging) {
                        // No drag occurred - treat as click
                        handleBubbleClick()
                    } else {
                        // Was dragging - check if dropped in close zone
                        if (isInCloseZone()) {
                            handleCloseBubble()
                        } else {
                            animateToEdge()
                        }
                    }
                    isDragEnabled = false
                    return true
                }
            }
            return false
        }

        /**
         * Check if bubble is within a threshold of the close zone.
         * Returns null if views are unavailable.
         */
        private fun isWithinCloseZoneThreshold(threshold: Double): Boolean {
            val bubble = bubbleView ?: return false
            val closeZone = closeZoneView ?: return false

            val bubbleLocation = IntArray(2)
            val closeZoneLocation = IntArray(2)
            bubble.getLocationOnScreen(bubbleLocation)
            closeZone.getLocationOnScreen(closeZoneLocation)

            val dx = (bubbleLocation[0] + bubble.width / 2) - (closeZoneLocation[0] + closeZone.width / 2)
            val dy = (bubbleLocation[1] + bubble.height / 2) - (closeZoneLocation[1] + closeZone.height / 2)
            val distance = kotlin.math.sqrt((dx * dx + dy * dy).toDouble())
            val combinedRadius = (closeZone.width / 2 + bubble.width / 2).toDouble()

            return distance < combinedRadius * threshold
        }

        private fun isInCloseZone(): Boolean = isWithinCloseZoneThreshold(0.7)

        private fun isNearCloseZone(): Boolean = isWithinCloseZoneThreshold(1.2)
    }

    private fun handleBubbleClick() {
        Log.d(TAG, "handleBubbleClick: Tap detected")

        if (FloatingBubblePlugin.isActivityResumed) {
            // App in foreground - emit event to WebView
            Log.d(TAG, "App in foreground - emitting click event")
            FloatingBubblePlugin.invokeBubbleClick()
        } else {
            // App backgrounded - toggle native capture
            Log.d(TAG, "App backgrounded - toggling native capture")
            if (FloatingBubblePlugin.isNativeCapture) {
                stopNativeCapture()
            } else {
                startNativeCapture()
            }
        }
    }

    private fun handleCloseBubble() {
        Log.d(TAG, "handleCloseBubble: Drag-to-close detected")

        // Stop native capture if active
        if (FloatingBubblePlugin.isNativeCapture) {
            Log.d(TAG, "Stopping native capture before close")
            stopNativeCapture()
        }

        FloatingBubblePlugin.invokeBubbleClose()
        hideBubble()
    }

    private fun hideBubble() {
        try {
            val intent = Intent(this, FloatingBubbleService::class.java)
            stopService(intent)
        } catch (e: Exception) {
            Log.e(TAG, "Error hiding bubble", e)
        }
    }

    private fun animateToEdge() {
        val screenWidth = resources.displayMetrics.widthPixels
        val bubbleWidth = bubbleView?.width ?: 0
        val currentX = layoutParams?.x ?: 0

        val targetX = if (currentX + bubbleWidth / 2 < screenWidth / 2) {
            0
        } else {
            screenWidth - bubbleWidth
        }

        layoutParams?.x = targetX
        windowManager?.updateViewLayout(bubbleView, layoutParams)
    }

    // ========== Native Audio Recording ==========

    /**
     * Initialize AudioRecord for native capture.
     * Uses 16kHz mono float32 to match backend expectations.
     */
    @Suppress("MissingPermission")
    private fun initAudioRecord(): Boolean {
        val minBufferSize = AudioRecord.getMinBufferSize(
            SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_FLOAT
        )

        if (minBufferSize == AudioRecord.ERROR_BAD_VALUE || minBufferSize == AudioRecord.ERROR) {
            Log.e(TAG, "Invalid AudioRecord parameters, minBufferSize=$minBufferSize")
            return false
        }

        try {
            audioRecord = AudioRecord(
                MediaRecorder.AudioSource.MIC,
                SAMPLE_RATE,
                AudioFormat.CHANNEL_IN_MONO,
                AudioFormat.ENCODING_PCM_FLOAT,
                minBufferSize * 2
            )
            val initialized = audioRecord?.state == AudioRecord.STATE_INITIALIZED
            if (!initialized) {
                Log.e(TAG, "AudioRecord failed to initialize")
                audioRecord?.release()
                audioRecord = null
            }
            return initialized
        } catch (e: SecurityException) {
            Log.e(TAG, "Microphone permission denied", e)
            return false
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create AudioRecord", e)
            return false
        }
    }

    /**
     * Execute the pending capture callback if one exists.
     * Called when consumer signals ready via JavascriptInterface.
     */
    private fun executePendingCaptureCallback() {
        readyTimeoutHandler.removeCallbacks(readyTimeoutRunnable)
        val callback = pendingCaptureCallback
        pendingCaptureCallback = null
        if (callback != null) {
            Log.d(TAG, "Executing pending capture callback - consumer is ready")
            callback()
        }
    }

    /**
     * Execute the pending stop callback if one exists.
     * Called when all data has been flushed via JavascriptInterface.
     */
    private fun executePendingStopCallback() {
        readyTimeoutHandler.removeCallbacks(flushTimeoutRunnable)
        val callback = pendingStopCallback
        pendingStopCallback = null
        if (callback != null) {
            Log.d(TAG, "Executing pending stop callback - data flushed")
            callback()
        }
    }

    /**
     * Start capturing audio natively.
     * Called when bubble is tapped while app is backgrounded.
     * Emits capture-start event and waits for consumer to signal ready.
     */
    fun startNativeCapture() {
        if (isCapturing) {
            Log.d(TAG, "startNativeCapture: Already capturing")
            return
        }

        if (!initAudioRecord()) {
            Log.e(TAG, "startNativeCapture: Failed to initialize AudioRecord")
            return
        }

        isCapturing = true
        audioRecord?.startRecording()

        // Update bubble state immediately
        updateState("capturing")
        FloatingBubblePlugin.isNativeCapture = true

        // Store the callback to start capture thread
        pendingCaptureCallback = {
            captureThread = Thread {
                Log.d(TAG, "Capture thread started")
                val buffer = FloatArray(CHUNK_SIZE)
                var chunkCount = 0
                while (isCapturing) {
                    val read = audioRecord?.read(buffer, 0, CHUNK_SIZE, AudioRecord.READ_BLOCKING) ?: 0
                    if (read > 0) {
                        chunkCount++

                        // Calculate audio level to verify real audio is being captured
                        var maxAmplitude = 0f
                        var sumSquares = 0.0
                        for (i in 0 until read) {
                            val absVal = kotlin.math.abs(buffer[i])
                            if (absVal > maxAmplitude) maxAmplitude = absVal
                            sumSquares += buffer[i] * buffer[i]
                        }
                        val rms = kotlin.math.sqrt(sumSquares / read).toFloat()

                        Log.d(TAG, "Audio chunk $chunkCount: read $read samples, max=$maxAmplitude, rms=$rms")

                        if (maxAmplitude < 0.001f) {
                            Log.w(TAG, "Audio chunk $chunkCount appears to be silence (max < 0.001)")
                        }

                        // Emit data event instead of direct command invocation
                        emitCaptureData(buffer.copyOf(read))
                    } else {
                        Log.w(TAG, "AudioRecord.read returned $read")
                    }
                }
                Log.d(TAG, "Capture thread stopped, sent $chunkCount chunks")
            }.apply {
                name = "NativeAudioCapture"
                start()
            }
        }

        // Emit capture-start event - consumer should call signalReady() when ready
        notifyCaptureStarted()

        // Timeout protection - start anyway after 3 seconds
        readyTimeoutHandler.postDelayed(readyTimeoutRunnable, 3000L)

        Log.d(TAG, "Native capture started, waiting for ready callback")
    }

    /**
     * Stop capturing audio.
     * Uses two-phase synchronization to ensure all data is processed
     * before emitting capture-stop event.
     */
    fun stopNativeCapture() {
        if (!isCapturing) {
            Log.d(TAG, "stopNativeCapture: Not capturing")
            return
        }

        isCapturing = false

        // Wait for capture thread to finish posting data
        try {
            captureThread?.join(1000)
        } catch (e: InterruptedException) {
            Log.w(TAG, "Interrupted while waiting for capture thread", e)
        }
        captureThread = null

        // Stop and release AudioRecord
        try {
            audioRecord?.stop()
        } catch (e: Exception) {
            Log.w(TAG, "Error stopping AudioRecord", e)
        }
        audioRecord?.release()
        audioRecord = null

        // Store callback to finalize after data is flushed
        pendingStopCallback = {
            notifyCaptureStopped()
            updateState("processing")
            FloatingBubblePlugin.isNativeCapture = false
            Log.d(TAG, "Native capture stopped")
        }

        // Send flush marker and wait for callback
        flushPendingData()

        // Timeout protection - stop anyway after 2 seconds
        readyTimeoutHandler.postDelayed(flushTimeoutRunnable, 2000L)

        Log.d(TAG, "Waiting for data to flush before stopping")
    }

    /**
     * Emit capture data event with audio samples.
     */
    private fun emitCaptureData(samples: FloatArray) {
        Log.d(TAG, "emitCaptureData: Emitting ${samples.size} samples")
        FloatingBubblePlugin.emitCaptureData("audio", samples)
    }

    /**
     * Send a flush marker to ensure all pending data is processed.
     * The marker travels through the same JS queue as data events,
     * so when it executes, all prior events have been sent.
     */
    private fun flushPendingData() {
        val webView = FloatingBubblePlugin.webViewInstance
        if (webView == null) {
            Log.w(TAG, "flushPendingData: WebView not available")
            executePendingStopCallback()
            return
        }

        // This JS executes in order after all pending evaluateJavascript calls
        val js = """
            (function() {
                console.log('[FloatingBubble] Flushing pending data');
                // By the time this runs, all prior evaluateJavascript calls have executed
                if (window.FloatingBubbleBridge && window.FloatingBubbleBridge.onFlushed) {
                    window.FloatingBubbleBridge.onFlushed();
                }
            })();
        """.trimIndent()

        Handler(Looper.getMainLooper()).post {
            try {
                webView.evaluateJavascript(js, null)
            } catch (e: Exception) {
                Log.e(TAG, "Error flushing data", e)
                executePendingStopCallback()
            }
        }
    }

    /**
     * Emit capture-start event.
     * Consumer should call signalReady() via FloatingBubbleBridge.onReady() when ready.
     */
    private fun notifyCaptureStarted() {
        Log.d(TAG, "notifyCaptureStarted: Emitting capture-start event")
        FloatingBubblePlugin.emitCaptureStart()
    }

    /**
     * Emit capture-stop event.
     * Consumer should handle processing completion and reset bubble state.
     */
    private fun notifyCaptureStopped() {
        Log.d(TAG, "notifyCaptureStopped: Emitting capture-stop event")
        FloatingBubblePlugin.emitCaptureStop()
    }

    /**
     * Update the visual state of the bubble.
     * Changes the icon based on state configuration.
     */
    private fun updateState(stateName: String) {
        if (currentStateName == stateName) return
        currentStateName = stateName

        // Determine icon: state-specific icon -> default icon -> system fallback
        val config = Companion.stateConfigs[stateName]
        val iconName = config?.iconResourceName ?: Companion.defaultIconResourceName

        if (iconName != null) {
            val iconResId = resources.getIdentifier(iconName, "drawable", packageName)
            if (iconResId != 0) {
                val iconDrawable = ContextCompat.getDrawable(this, iconResId)
                bubbleView?.setImageDrawable(iconDrawable)
            } else {
                Log.w(TAG, "State icon resource not found: $iconName")
            }
        }

        // Update notification
        val notificationManager = getSystemService(NotificationManager::class.java)
        notificationManager.notify(NOTIFICATION_ID, createNotification())
    }

    private fun createNotification(): Notification {
        // Check for configured notification content first
        val configuredContent = Companion.notificationConfig
            ?.stateNotifications
            ?.get(currentStateName)

        val (title, text) = if (configuredContent != null) {
            configuredContent.title to configuredContent.text
        } else {
            // Generic defaults
            when (currentStateName) {
                "capturing" -> "Capturing..." to "Tap bubble to stop"
                "processing" -> "Processing..." to "Working on your input"
                else -> "Floating Bubble" to "Tap the bubble to interact"
            }
        }

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(text)
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setOngoing(true)
            .build()
    }
}
