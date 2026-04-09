package se.olivermarcusson.euripus.receiver.data.player

import androidx.media3.common.Player
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PlaybackStateHeuristicsTest {
    @Test
    fun `buffering playback is not treated as paused`() {
        assertTrue(
            PlaybackStateHeuristics.isBuffering(
                hasSource = true,
                playWhenReady = true,
                playbackState = Player.STATE_BUFFERING,
                hasError = false,
            ),
        )
        assertFalse(
            PlaybackStateHeuristics.isPaused(
                hasSource = true,
                playWhenReady = true,
                playbackState = Player.STATE_BUFFERING,
                hasError = false,
            ),
        )
    }

    @Test
    fun `errored playback is treated as paused`() {
        assertTrue(
            PlaybackStateHeuristics.isPaused(
                hasSource = true,
                playWhenReady = true,
                playbackState = Player.STATE_READY,
                hasError = true,
            ),
        )
    }

    @Test
    fun `seek completion uses tolerance and ignores buffering`() {
        assertTrue(
            PlaybackStateHeuristics.seekCompleted(
                targetSeconds = 120.0,
                positionSeconds = 121.0,
                buffering = false,
                toleranceSeconds = 1.5,
            ),
        )
        assertFalse(
            PlaybackStateHeuristics.seekCompleted(
                targetSeconds = 120.0,
                positionSeconds = 121.0,
                buffering = true,
                toleranceSeconds = 1.5,
            ),
        )
    }
}
