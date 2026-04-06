package se.olivermarcusson.euripus.receiver.domain

import se.olivermarcusson.euripus.receiver.data.api.PlaybackSourceDto

data class ReceiverUiState(
    val status: ReceiverStatus = ReceiverStatus.STARTING_SESSION,
    val configuredServerOrigin: String? = null,
    val serverInput: String = "",
    val pairingCode: String? = null,
    val source: PlaybackSourceDto? = null,
    val errorMessage: String? = null,
    val detailMessage: String? = null,
    val isBusy: Boolean = false,
)
