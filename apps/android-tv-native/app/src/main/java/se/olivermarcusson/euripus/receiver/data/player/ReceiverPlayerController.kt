package se.olivermarcusson.euripus.receiver.data.player

import android.content.Context
import android.util.Log
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.hls.HlsMediaSource
import androidx.media3.exoplayer.source.ProgressiveMediaSource
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import se.olivermarcusson.euripus.receiver.data.api.PlaybackSourceDto

class ReceiverPlayerController(
    private val context: Context,
) {
    companion object {
        private const val TAG = "ReceiverPlayer"
        private const val MIN_BUFFER_MS = 1_500
        private const val MAX_BUFFER_MS = 6_000
        private const val BUFFER_FOR_PLAYBACK_MS = 500
        private const val BUFFER_FOR_REBUFFER_MS = 1_000
        private const val CONNECT_TIMEOUT_MS = 10_000
        private const val READ_TIMEOUT_MS = 20_000
        private const val PLAYER_USER_AGENT = "EuripusReceiverAndroidTv/1.0"
        private const val LIVE_TARGET_OFFSET_MS = 2_500L
        private const val LIVE_MIN_OFFSET_MS = 1_500L
        private const val LIVE_MAX_OFFSET_MS = 5_000L
        private const val LIVE_HARD_RESYNC_OFFSET_MS = 6_000L
        private const val LIVE_CATCHUP_OFFSET_MS = 3_500L
        private const val LIVE_CATCHUP_PLAYBACK_SPEED = 1.03f
    }

    private val mutableSnapshot = MutableStateFlow(PlaybackSnapshot())
    private val mutableCurrentSource = MutableStateFlow<PlaybackSourceDto?>(null)

    val player: ExoPlayer = ExoPlayer.Builder(context)
        .setLoadControl(
            DefaultLoadControl.Builder()
                .setBufferDurationsMs(
                    MIN_BUFFER_MS,
                    MAX_BUFFER_MS,
                    BUFFER_FOR_PLAYBACK_MS,
                    BUFFER_FOR_REBUFFER_MS,
                )
                .build(),
        )
        .build().apply {
        repeatMode = Player.REPEAT_MODE_OFF
        playWhenReady = true
        addListener(
            object : Player.Listener {
                override fun onEvents(player: Player, events: Player.Events) {
                    syncLivePlaybackPosition()
                    publishSnapshot()
                }
            },
        )
    }

    val currentSource: StateFlow<PlaybackSourceDto?> = mutableCurrentSource.asStateFlow()
    val snapshot: StateFlow<PlaybackSnapshot> = mutableSnapshot.asStateFlow()

    fun setSource(source: PlaybackSourceDto?) {
        mutableCurrentSource.value = source
        if (source == null || source.kind == "unsupported") {
            stopPlayback()
            return
        }

        val mediaItem = MediaItem.Builder()
            .setUri(source.url)
            .setMimeType(
                when (source.kind) {
                    "hls" -> MimeTypes.APPLICATION_M3U8
                    else -> MimeTypes.VIDEO_MP2T
                },
            )
            .apply {
                if (source.live) {
                    setLiveConfiguration(
                        MediaItem.LiveConfiguration.Builder()
                            .setTargetOffsetMs(LIVE_TARGET_OFFSET_MS)
                            .setMinOffsetMs(LIVE_MIN_OFFSET_MS)
                            .setMaxOffsetMs(LIVE_MAX_OFFSET_MS)
                            .setMaxPlaybackSpeed(LIVE_CATCHUP_PLAYBACK_SPEED)
                            .build(),
                    )
                }
            }
            .build()

        val mediaSource = when (source.kind) {
            "hls" -> HlsMediaSource.Factory(createDataSourceFactory(source)).createMediaSource(mediaItem)
            else -> ProgressiveMediaSource.Factory(createDataSourceFactory(source)).createMediaSource(mediaItem)
        }

        player.setMediaSource(mediaSource)
        player.prepare()
        player.playWhenReady = true
        publishSnapshot()
    }

    fun play() {
        player.play()
        syncLivePlaybackPosition(force = true)
        publishSnapshot()
    }

    fun playFromTvRemote() {
        val source = mutableCurrentSource.value
        Log.d(TAG, "playFromTvRemote live=${source?.live} mediaItemCount=${player.mediaItemCount}")
        if (source?.live == true && player.mediaItemCount > 0) {
            player.seekToDefaultPosition()
        }
        player.play()
        publishSnapshot()
    }

    fun togglePlayPauseFromTvRemote() {
        if (mutableCurrentSource.value == null || player.mediaItemCount == 0) {
            Log.d(TAG, "togglePlayPauseFromTvRemote ignored source=${mutableCurrentSource.value != null} mediaItemCount=${player.mediaItemCount}")
            return
        }

        Log.d(TAG, "togglePlayPauseFromTvRemote isPlaying=${player.isPlaying}")
        if (player.isPlaying) {
            pause()
        } else {
            playFromTvRemote()
        }
    }

    fun pause() {
        player.pause()
        resetPlaybackParameters()
        publishSnapshot()
    }

    fun stopPlayback() {
        mutableCurrentSource.value = null
        clearPlayerOnly()
        publishSnapshot()
    }

    fun clearPlayerOnly() {
        player.stop()
        player.clearMediaItems()
        resetPlaybackParameters()
    }

    fun markUnsupported(source: PlaybackSourceDto) {
        mutableCurrentSource.value = source
        clearPlayerOnly()
        publishSnapshot()
    }

    fun seekTo(positionSeconds: Double) {
        player.seekTo((positionSeconds * 1000).toLong().coerceAtLeast(0L))
        syncLivePlaybackPosition()
        publishSnapshot()
    }

    fun release() {
        player.release()
    }

    private fun createDataSourceFactory(source: PlaybackSourceDto): DataSource.Factory {
        val httpFactory = DefaultHttpDataSource.Factory()
            .setAllowCrossProtocolRedirects(true)
            .setConnectTimeoutMs(CONNECT_TIMEOUT_MS)
            .setReadTimeoutMs(READ_TIMEOUT_MS)
            .setUserAgent(PLAYER_USER_AGENT)
            .setDefaultRequestProperties(source.headers)
        return DefaultDataSource.Factory(context, httpFactory)
    }

    private fun publishSnapshot() {
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it >= 0 }?.div(1000.0)
        mutableSnapshot.value = PlaybackSnapshot(
            paused = !player.isPlaying,
            positionSeconds = player.currentPosition.takeIf { it >= 0 }?.div(1000.0),
            durationSeconds = duration,
        )
    }

    private fun syncLivePlaybackPosition(force: Boolean = false) {
        val source = mutableCurrentSource.value ?: return
        if (!source.live || player.mediaItemCount == 0) {
            resetPlaybackParameters()
            return
        }

        val liveOffsetMs = player.currentLiveOffset
        if (liveOffsetMs == C.TIME_UNSET || liveOffsetMs < 0) {
            resetPlaybackParameters()
            return
        }

        if (force || liveOffsetMs > LIVE_HARD_RESYNC_OFFSET_MS) {
            Log.d(TAG, "syncLivePlaybackPosition forcing live edge liveOffsetMs=$liveOffsetMs")
            player.seekToDefaultPosition()
            player.playWhenReady = true
            resetPlaybackParameters()
            return
        }

        if (!player.isPlaying) {
            resetPlaybackParameters()
            return
        }

        val desiredSpeed = if (liveOffsetMs > LIVE_CATCHUP_OFFSET_MS) {
            LIVE_CATCHUP_PLAYBACK_SPEED
        } else {
            1f
        }
        if (player.playbackParameters.speed != desiredSpeed) {
            player.playbackParameters = PlaybackParameters(desiredSpeed)
        }
    }

    private fun resetPlaybackParameters() {
        if (player.playbackParameters.speed != 1f) {
            player.playbackParameters = PlaybackParameters(1f)
        }
    }
}
