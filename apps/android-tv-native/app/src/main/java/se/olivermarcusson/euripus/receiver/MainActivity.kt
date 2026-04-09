package se.olivermarcusson.euripus.receiver

import android.os.Bundle
import android.view.KeyEvent
import android.view.Display
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import se.olivermarcusson.euripus.receiver.session.ReceiverViewModel
import se.olivermarcusson.euripus.receiver.ui.ReceiverApp

class MainActivity : ComponentActivity() {
    companion object {
        private const val TAG = "MainActivity"
    }

    private val viewModel by viewModels<ReceiverViewModel>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        applySmoothPlaybackDisplayModePreference()
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior =
                WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }

        setContent {
            ReceiverApp(viewModel = viewModel)
        }
    }

    override fun onStart() {
        super.onStart()
        applySmoothPlaybackDisplayModePreference()
        viewModel.onForegroundChanged(true)
    }

    override fun onStop() {
        viewModel.onForegroundChanged(false)
        super.onStop()
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        Log.d(
            TAG,
            "dispatchKeyEvent action=${event.action} keyCode=${event.keyCode} repeat=${event.repeatCount} source=${event.source} deviceId=${event.deviceId}",
        )

        if (event.action == KeyEvent.ACTION_DOWN &&
            event.repeatCount == 0 &&
            viewModel.handleHardwareKey(event.keyCode)
        ) {
            return true
        }

        return super.dispatchKeyEvent(event)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (event.repeatCount == 0 && viewModel.handleHardwareKey(keyCode)) {
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    private fun applySmoothPlaybackDisplayModePreference() {
        val activeDisplay = display ?: return
        val currentMode = activeDisplay.mode ?: return
        val supportedModes = activeDisplay.supportedModes.orEmpty()
        if (supportedModes.isEmpty()) {
            return
        }

        val preferredMode = choosePreferredDisplayMode(
            currentMode = currentMode,
            supportedModes = supportedModes,
        ) ?: return

        if (preferredMode.modeId == currentMode.modeId) {
            return
        }

        val attributes = window.attributes
        if (attributes.preferredDisplayModeId == preferredMode.modeId) {
            return
        }

        attributes.preferredDisplayModeId = preferredMode.modeId
        window.attributes = attributes
        Log.i(
            TAG,
            "Requested smoother display mode ${preferredMode.physicalWidth}x${preferredMode.physicalHeight}@${preferredMode.refreshRate}Hz " +
                "(current ${currentMode.physicalWidth}x${currentMode.physicalHeight}@${currentMode.refreshRate}Hz)",
        )
    }

    private fun choosePreferredDisplayMode(
        currentMode: Display.Mode,
        supportedModes: Array<out Display.Mode>,
    ): Display.Mode? {
        val sameResolutionHigherRefresh = supportedModes
            .filter { mode ->
                mode.physicalWidth == currentMode.physicalWidth &&
                    mode.physicalHeight == currentMode.physicalHeight &&
                    mode.refreshRate > currentMode.refreshRate + 0.5f
            }
            .maxByOrNull { mode -> mode.refreshRate }

        // Android-x86 on external TVs often boots into 4K30 even when smoother 1080p60 modes exist.
        val smoother1080pMode = supportedModes
            .filter { mode ->
                mode.physicalWidth <= 1920 &&
                    mode.physicalHeight <= 1080 &&
                    mode.refreshRate >= 50f
            }
            .maxWithOrNull(
                compareByDescending<Display.Mode> { it.refreshRate }
                    .thenByDescending { it.physicalWidth * it.physicalHeight },
            )

        return when {
            currentMode.physicalWidth >= 3840 &&
                currentMode.refreshRate <= 30.5f &&
                smoother1080pMode != null -> smoother1080pMode
            sameResolutionHigherRefresh != null -> sameResolutionHigherRefresh
            else -> null
        }
    }
}
