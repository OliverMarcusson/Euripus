package se.olivermarcusson.euripus.receiver.data.api

import kotlinx.serialization.Serializable

@Serializable
data class ApiError(
    val error: String,
    val message: String,
    val status: Int,
)

@Serializable
data class PlaybackSourceDto(
    val kind: String,
    val url: String,
    val headers: Map<String, String> = emptyMap(),
    val live: Boolean,
    val catchup: Boolean,
    val expiresAt: String? = null,
    val unsupportedReason: String? = null,
    val title: String,
)

@Serializable
data class ReceiverPlaybackStateDto(
    val title: String,
    val sourceKind: String,
    val live: Boolean,
    val catchup: Boolean,
    val updatedAt: String,
    val paused: Boolean,
    val buffering: Boolean,
    val positionSeconds: Double? = null,
    val durationSeconds: Double? = null,
    val errorMessage: String? = null,
)

@Serializable
data class ReceiverDeviceDto(
    val id: String,
    val name: String,
    val platform: String,
    val formFactorHint: String? = null,
    val appKind: String,
    val remembered: Boolean,
    val online: Boolean,
    val currentController: Boolean,
    val lastSeenAt: String,
    val updatedAt: String,
    val currentPlayback: ReceiverPlaybackStateDto? = null,
)

@Serializable
data class RemotePlaybackCommandDto(
    val id: String,
    val targetDeviceId: String,
    val targetDeviceName: String,
    val commandType: String,
    val status: String,
    val sourceTitle: String,
    val createdAt: String,
)

@Serializable
data class ReceiverEventPayloadDto(
    val eventType: String,
    val command: RemotePlaybackCommandDto,
    val source: PlaybackSourceDto? = null,
    val positionSeconds: Double? = null,
    val receiverCredential: String? = null,
)

@Serializable
data class ReceiverSessionPayloadDto(
    val deviceKey: String,
    val name: String,
    val platform: String,
    val formFactorHint: String? = null,
    val appKind: String,
    val publicOrigin: String? = null,
    val receiverCredential: String? = null,
)

@Serializable
data class ReceiverSessionResponseDto(
    val sessionToken: String,
    val expiresAt: String,
    val receiverCredential: String? = null,
    val device: ReceiverDeviceDto,
    val pairingCode: String? = null,
    val paired: Boolean,
)

@Serializable
data class ReceiverPairingCodeDto(
    val code: String,
    val expiresAt: String,
    val device: ReceiverDeviceDto,
)

@Serializable
data class ReceiverPlaybackStatePayloadDto(
    val title: String? = null,
    val sourceKind: String? = null,
    val live: Boolean? = null,
    val catchup: Boolean? = null,
    val paused: Boolean? = null,
    val buffering: Boolean? = null,
    val positionSeconds: Double? = null,
    val durationSeconds: Double? = null,
    val errorMessage: String? = null,
)

@Serializable
data class RemoteCommandAckDto(
    val status: String,
    val errorMessage: String? = null,
)
