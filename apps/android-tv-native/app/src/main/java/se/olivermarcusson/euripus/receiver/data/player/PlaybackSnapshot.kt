package se.olivermarcusson.euripus.receiver.data.player

data class PlaybackSnapshot(
    val paused: Boolean = true,
    val buffering: Boolean = false,
    val positionSeconds: Double? = null,
    val durationSeconds: Double? = null,
    val errorMessage: String? = null,
)
