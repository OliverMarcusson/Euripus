package se.olivermarcusson.euripus.receiver

import android.os.Bundle
import android.view.KeyEvent
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
}
