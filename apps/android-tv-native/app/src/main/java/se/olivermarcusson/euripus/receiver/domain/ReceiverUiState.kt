package se.olivermarcusson.euripus.receiver.domain

import se.olivermarcusson.euripus.receiver.data.api.PlaybackSourceDto
import se.olivermarcusson.euripus.receiver.data.api.ReceiverFavoriteChannelEntryDto

data class ReceiverUiState(
    val status: ReceiverStatus = ReceiverStatus.STARTING_SESSION,
    val configuredServerOrigin: String? = null,
    val serverInput: String = "",
    val pairingCode: String? = null,
    val source: PlaybackSourceDto? = null,
    val errorMessage: String? = null,
    val detailMessage: String? = null,
    val isBusy: Boolean = false,
    val channelViewerOpen: Boolean = false,
    val channelViewerLoading: Boolean = false,
    val channelViewerError: String? = null,
    val favoriteChannels: List<ReceiverFavoriteChannelEntryDto> = emptyList(),
    val selectedChannelIndex: Int = 0,
    val tuningChannelId: String? = null,
)
