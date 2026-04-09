package se.olivermarcusson.euripus.receiver.data.player

import androidx.media3.common.Player
import kotlin.math.abs

object PlaybackStateHeuristics {
    fun isBuffering(
        hasSource: Boolean,
        playWhenReady: Boolean,
        playbackState: Int,
        hasError: Boolean,
    ): Boolean = hasSource && playWhenReady && playbackState == Player.STATE_BUFFERING && !hasError

    fun isPaused(
        hasSource: Boolean,
        playWhenReady: Boolean,
        playbackState: Int,
        hasError: Boolean,
    ): Boolean {
        if (!hasSource || hasError) {
            return true
        }
        if (isBuffering(hasSource, playWhenReady, playbackState, hasError)) {
            return false
        }
        if (playbackState == Player.STATE_ENDED) {
            return true
        }
        return !playWhenReady
    }

    fun seekCompleted(
        targetSeconds: Double?,
        positionSeconds: Double?,
        buffering: Boolean,
        toleranceSeconds: Double,
    ): Boolean {
        if (targetSeconds == null || positionSeconds == null || buffering) {
            return false
        }
        return abs(positionSeconds - targetSeconds) <= toleranceSeconds
    }
}
