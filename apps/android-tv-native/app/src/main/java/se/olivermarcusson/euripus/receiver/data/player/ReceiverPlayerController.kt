package se.olivermarcusson.euripus.receiver.data.player

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
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
        private const val MIN_BUFFER_MS = 8_000
        private const val MAX_BUFFER_MS = 24_000
        private const val BUFFER_FOR_PLAYBACK_MS = 2_500
        private const val BUFFER_FOR_REBUFFER_MS = 4_000
        private const val CONNECT_TIMEOUT_MS = 10_000
        private const val READ_TIMEOUT_MS = 20_000
        private const val PLAYER_USER_AGENT = "EuripusReceiverAndroidTv/1.0"
        private const val LIVE_TARGET_OFFSET_MS = 8_000L
        private const val LIVE_MIN_OFFSET_MS = 5_000L
        private const val LIVE_MAX_OFFSET_MS = 18_000L
        private const val LIVE_CATCHUP_OFFSET_MS = 10_000L
        private const val LIVE_CATCHUP_PLAYBACK_SPEED = 1.03f
        private const val SNAPSHOT_SAMPLE_INTERVAL_MS = 2_000L
    }

    private val mutableSnapshot = MutableStateFlow(PlaybackSnapshot())
    private val mutableCurrentSource = MutableStateFlow<PlaybackSourceDto?>(null)
    private val snapshotHandler = Handler(Looper.getMainLooper())
    private var released = false
    private var lastErrorMessage: String? = null
    private val snapshotSampler = object : Runnable {
        override fun run() {
            if (released) {
                return
            }
            if (shouldSamplePlayback()) {
                publishSnapshot()
            }
            snapshotHandler.postDelayed(this, SNAPSHOT_SAMPLE_INTERVAL_MS)
        }
    }

    private val renderersFactory = DefaultRenderersFactory(context)
        .setEnableDecoderFallback(true)

    val player: ExoPlayer = ExoPlayer.Builder(context)
        .setRenderersFactory(renderersFactory)
        .setLoadControl(
            DefaultLoadControl.Builder()
                .setBufferDurationsMs(
                    MIN_BUFFER_MS,
                    MAX_BUFFER_MS,
                    BUFFER_FOR_PLAYBACK_MS,
                    BUFFER_FOR_REBUFFER_MS,
                )
                .setPrioritizeTimeOverSizeThresholds(true)
                .build(),
        )
        .build().apply {
            repeatMode = Player.REPEAT_MODE_OFF
            playWhenReady = true
            setVideoChangeFrameRateStrategy(C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS)
            addListener(
                object : Player.Listener {
                    override fun onEvents(player: Player, events: Player.Events) {
                        syncLivePlaybackPosition()
                        publishSnapshot()
                    }

                    override fun onPlayerError(error: PlaybackException) {
                        lastErrorMessage = error.localizedMessage ?: "Playback failed."
                        Log.w(TAG, "Player error", error)
                        publishSnapshot()
                    }
                },
            )
        }

    val currentSource: StateFlow<PlaybackSourceDto?> = mutableCurrentSource.asStateFlow()
    val snapshot: StateFlow<PlaybackSnapshot> = mutableSnapshot.asStateFlow()

    init {
        snapshotHandler.post(snapshotSampler)
    }

    fun setSource(source: PlaybackSourceDto?) {
        mutableCurrentSource.value = source
        lastErrorMessage = null
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
        if (source.live) {
            player.seekToDefaultPosition()
        }
        player.playWhenReady = true
        publishSnapshot()
    }

    fun play() {
        player.play()
        syncLivePlaybackPosition()
        publishSnapshot()
    }

    fun playFromTvRemote() {
        Log.d(
            TAG,
            "playFromTvRemote live=${mutableCurrentSource.value?.live} mediaItemCount=${player.mediaItemCount}",
        )
        player.play()
        syncLivePlaybackPosition()
        publishSnapshot()
    }

    fun togglePlayPauseFromTvRemote() {
        if (mutableCurrentSource.value == null || player.mediaItemCount == 0) {
            Log.d(
                TAG,
                "togglePlayPauseFromTvRemote ignored source=${mutableCurrentSource.value != null} mediaItemCount=${player.mediaItemCount}",
            )
            return
        }

        Log.d(TAG, "togglePlayPauseFromTvRemote isPlaying=${player.isPlaying}")
        if (isPausedForUi()) {
            playFromTvRemote()
        } else {
            pause()
        }
    }

    fun pause() {
        player.pause()
        resetPlaybackParameters()
        publishSnapshot()
    }

    fun stopPlayback() {
        mutableCurrentSource.value = null
        lastErrorMessage = null
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
        lastErrorMessage = source.unsupportedReason ?: "This stream is not supported on the receiver."
        clearPlayerOnly()
        publishSnapshot()
    }

    fun seekTo(positionSeconds: Double) {
        player.seekTo((positionSeconds * 1000).toLong().coerceAtLeast(0L))
        syncLivePlaybackPosition()
        publishSnapshot()
    }

    fun isReadyForPlayback(): Boolean =
        mutableCurrentSource.value != null &&
            lastErrorMessage == null &&
            player.mediaItemCount > 0 &&
            player.playbackState == Player.STATE_READY

    fun refreshSnapshot() {
        publishSnapshot()
    }

    fun release() {
        released = true
        snapshotHandler.removeCallbacksAndMessages(null)
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

    private fun shouldSamplePlayback(): Boolean =
        mutableCurrentSource.value != null || player.mediaItemCount > 0 || lastErrorMessage != null

    private fun publishSnapshot() {
        val hasSource = mutableCurrentSource.value != null && player.mediaItemCount > 0
        val duration = player.duration.takeIf { it != C.TIME_UNSET && it >= 0 }?.div(1000.0)
        val nextSnapshot = PlaybackSnapshot(
            paused = PlaybackStateHeuristics.isPaused(
                hasSource = hasSource,
                playWhenReady = player.playWhenReady,
                playbackState = player.playbackState,
                hasError = lastErrorMessage != null,
            ),
            buffering = PlaybackStateHeuristics.isBuffering(
                hasSource = hasSource,
                playWhenReady = player.playWhenReady,
                playbackState = player.playbackState,
                hasError = lastErrorMessage != null,
            ),
            positionSeconds = player.currentPosition.takeIf { hasSource && it >= 0 }?.div(1000.0),
            durationSeconds = duration,
            errorMessage = lastErrorMessage,
        )
        if (mutableSnapshot.value != nextSnapshot) {
            mutableSnapshot.value = nextSnapshot
        }
    }

    private fun isPausedForUi(): Boolean {
        val hasSource = mutableCurrentSource.value != null && player.mediaItemCount > 0
        if (!hasSource) {
            return true
        }

        return PlaybackStateHeuristics.isPaused(
            hasSource = hasSource,
            playWhenReady = player.playWhenReady,
            playbackState = player.playbackState,
            hasError = lastErrorMessage != null,
        )
    }

    private fun syncLivePlaybackPosition() {
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
