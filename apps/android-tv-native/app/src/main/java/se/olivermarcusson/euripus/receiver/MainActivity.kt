package se.olivermarcusson.euripus.receiver

import android.app.Activity
import android.os.Bundle
import android.widget.FrameLayout
import android.widget.TextView

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val message = TextView(this).apply {
            text = "Euripus Receiver\n\nNative Android TV receiver scaffold.\nPairing, playback, and Media3 wiring live here."
            textSize = 24f
            setPadding(48, 48, 48, 48)
        }

        setContentView(FrameLayout(this).apply { addView(message) })
    }
}
