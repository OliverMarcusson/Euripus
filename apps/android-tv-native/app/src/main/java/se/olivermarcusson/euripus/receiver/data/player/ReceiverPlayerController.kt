package se.olivermarcusson.euripus.receiver.data.player

import android.content.Context
import android.util.Log
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.Player
import androidx.media3.datasource.DataSource
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
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
    }

    private val mutableSnapshot = MutableStateFlow(PlaybackSnapshot())
    private val mutableCurrentSource = MutableStateFlow<PlaybackSourceDto?>(null)

    val player: ExoPlayer = ExoPlayer.Builder(context).build().apply {
        repeatMode = Player.REPEAT_MODE_OFF
        playWhenReady = true
        addListener(
            object : Player.Listener {
                override fun onEvents(player: Player, events: Player.Events) {
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
    }

    fun markUnsupported(source: PlaybackSourceDto) {
        mutableCurrentSource.value = source
        clearPlayerOnly()
        publishSnapshot()
    }

    fun seekTo(positionSeconds: Double) {
        player.seekTo((positionSeconds * 1000).toLong().coerceAtLeast(0L))
        publishSnapshot()
    }

    fun release() {
        player.release()
    }

    private fun createDataSourceFactory(source: PlaybackSourceDto): DataSource.Factory {
        val httpFactory = DefaultHttpDataSource.Factory()
            .setAllowCrossProtocolRedirects(true)
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
}
