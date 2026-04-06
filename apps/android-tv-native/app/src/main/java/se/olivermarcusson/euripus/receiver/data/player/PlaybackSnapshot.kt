package se.olivermarcusson.euripus.receiver.data.player

data class PlaybackSnapshot(
    val paused: Boolean = true,
    val positionSeconds: Double? = null,
    val durationSeconds: Double? = null,
)
