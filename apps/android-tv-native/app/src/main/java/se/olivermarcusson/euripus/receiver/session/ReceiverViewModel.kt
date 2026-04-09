package se.olivermarcusson.euripus.receiver.session

import android.app.Application
import android.util.Log
import android.view.KeyEvent
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlin.math.min
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import se.olivermarcusson.euripus.receiver.config.ReceiverEndpointConfig
import se.olivermarcusson.euripus.receiver.config.normalizeServerOrigin
import se.olivermarcusson.euripus.receiver.data.api.PlaybackSourceDto
import se.olivermarcusson.euripus.receiver.data.api.ReceiverApiException
import se.olivermarcusson.euripus.receiver.data.api.ReceiverApiService
import se.olivermarcusson.euripus.receiver.data.api.ReceiverEventPayloadDto
import se.olivermarcusson.euripus.receiver.data.api.ReceiverPlaybackStatePayloadDto
import se.olivermarcusson.euripus.receiver.data.api.ReceiverSessionPayloadDto
import se.olivermarcusson.euripus.receiver.data.api.RemoteCommandAckDto
import se.olivermarcusson.euripus.receiver.data.events.ReceiverAuthExpiredException
import se.olivermarcusson.euripus.receiver.data.events.ReceiverEventStream
import se.olivermarcusson.euripus.receiver.data.player.PlaybackStateHeuristics
import se.olivermarcusson.euripus.receiver.data.player.ReceiverPlayerController
import se.olivermarcusson.euripus.receiver.data.storage.ReceiverPreferences
import se.olivermarcusson.euripus.receiver.data.storage.ReceiverPreferencesRepository
import se.olivermarcusson.euripus.receiver.domain.ReceiverStatus
import se.olivermarcusson.euripus.receiver.domain.ReceiverUiState

private const val TAG = "ReceiverViewModel"
private const val HEARTBEAT_INTERVAL_MS = 15_000L
private const val PLAYBACK_SYNC_INTERVAL_MS = 5_000L
private const val SEEK_COMPLETION_TOLERANCE_SECONDS = 1.5

private enum class PendingCommandKind {
    PLAYBACK_SOURCE,
    PLAY,
    PAUSE,
    SEEK,
    STOP,
}

private data class PendingCommand(
    val id: String,
    val kind: PendingCommandKind,
    val seekPositionSeconds: Double? = null,
)

class ReceiverViewModel(application: Application) : AndroidViewModel(application) {
    private val preferencesRepository = ReceiverPreferencesRepository(application)
    private val apiService = ReceiverApiService()
    private val eventStream = ReceiverEventStream(
        apiService = apiService,
        json = Json {
            ignoreUnknownKeys = true
            explicitNulls = false
        },
    )
    private val playerController = ReceiverPlayerController(application)

    private val mutableUiState = MutableStateFlow(ReceiverUiState())
    val uiState: StateFlow<ReceiverUiState> = mutableUiState.asStateFlow()
    val player = playerController.player

    private var currentPreferences: ReceiverPreferences = ReceiverPreferences()
    private var endpointConfig: ReceiverEndpointConfig? = null
    private var sessionToken: String? = null
    private var isForeground = false
    private var bootstrapJob: Job? = null
    private var heartbeatJob: Job? = null
    private var eventJob: Job? = null
    private var playbackSyncJob: Job? = null
    private var pendingCommand: PendingCommand? = null

    init {
        viewModelScope.launch {
            preferencesRepository.preferences.collectLatest { prefs ->
                val serverOriginChanged = prefs.serverOrigin != currentPreferences.serverOrigin
                currentPreferences = prefs
                mutableUiState.update { state ->
                    state.copy(
                        configuredServerOrigin = prefs.serverOrigin,
                        serverInput = if (state.serverInput.isBlank()) {
                            prefs.serverOrigin.orEmpty()
                        } else {
                            state.serverInput
                        },
                    )
                }
                if (isForeground && serverOriginChanged) {
                    requestBootstrap(force = true)
                }
            }
        }

        viewModelScope.launch {
            playerController.currentSource.collectLatest { source ->
                syncUiFromPlaybackState(source = source)
                runCatching { maybeCompletePendingCommand() }
                    .onFailure { error -> Log.w(TAG, "Failed to finish pending command", error) }
            }
        }

        viewModelScope.launch {
            playerController.snapshot.collectLatest {
                syncUiFromPlaybackState()
                runCatching { maybeCompletePendingCommand() }
                    .onFailure { error -> Log.w(TAG, "Failed to finish pending command", error) }
            }
        }
    }

    fun onForegroundChanged(isForeground: Boolean) {
        this.isForeground = isForeground
        if (isForeground) {
            requestBootstrap(force = false)
        } else {
            bootstrapJob?.cancel()
            bootstrapJob = null
            cancelSessionLoops()
            playerController.pause()
        }
    }

    fun onServerInputChanged(value: String) {
        mutableUiState.update {
            it.copy(serverInput = value, errorMessage = null, detailMessage = null)
        }
    }

    fun saveServerAndConnect() {
        viewModelScope.launch {
            val config = runCatching { normalizeServerOrigin(uiState.value.serverInput) }.getOrElse { error ->
                mutableUiState.update {
                    it.copy(
                        status = ReceiverStatus.NEEDS_SERVER_CONFIG,
                        errorMessage = error.message ?: "Enter a valid server URL.",
                        isBusy = false,
                    )
                }
                return@launch
            }

            mutableUiState.update {
                it.copy(
                    status = ReceiverStatus.STARTING_SESSION,
                    errorMessage = null,
                    detailMessage = "Checking the server and starting a receiver session.",
                    isBusy = true,
                )
            }

            runCatching {
                apiService.validateServer(config)
                preferencesRepository.saveServerOrigin(config.publicOrigin)
            }.onFailure { error ->
                mutableUiState.update {
                    it.copy(
                        status = ReceiverStatus.NEEDS_SERVER_CONFIG,
                        errorMessage = error.message ?: "Could not reach that Euripus server.",
                        detailMessage = "Make sure the URL points at the public Euripus web origin.",
                        isBusy = false,
                    )
                }
            }
        }
    }

    fun retry() {
        requestBootstrap(force = true)
    }

    private fun requestBootstrap(force: Boolean) {
        val existingJob = bootstrapJob
        if (force) {
            existingJob?.cancel()
        } else if (existingJob?.isActive == true) {
            return
        }

        val job = viewModelScope.launch {
            bootstrapReceiver(force)
        }
        bootstrapJob = job
        job.invokeOnCompletion {
            if (bootstrapJob === job) {
                bootstrapJob = null
            }
        }
    }

    fun refreshPairingCode() {
        val config = endpointConfig ?: return
        val token = sessionToken ?: return
        viewModelScope.launch {
            runCatching { apiService.issuePairingCode(config, token) }
                .onSuccess { pairing ->
                    mutableUiState.update {
                        it.copy(
                            status = ReceiverStatus.PAIRING,
                            pairingCode = pairing.code,
                            errorMessage = null,
                            detailMessage = "Open Euripus on your phone and enter the code below.",
                        )
                    }
                }
                .onFailure { error ->
                    mutableUiState.update {
                        it.copy(errorMessage = error.message ?: "Failed to refresh the pairing code.")
                    }
                }
        }
    }

    fun handleHardwareKey(keyCode: Int): Boolean = when (keyCode) {
        KeyEvent.KEYCODE_MEDIA_PLAY -> {
            playerController.playFromTvRemote()
            true
        }

        KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
        KeyEvent.KEYCODE_DPAD_CENTER,
        KeyEvent.KEYCODE_ENTER,
        KeyEvent.KEYCODE_NUMPAD_ENTER,
        KeyEvent.KEYCODE_SPACE,
        KeyEvent.KEYCODE_BUTTON_A,
        KeyEvent.KEYCODE_BUTTON_SELECT,
        KeyEvent.KEYCODE_HEADSETHOOK -> {
            playerController.togglePlayPauseFromTvRemote()
            true
        }

        KeyEvent.KEYCODE_MEDIA_PAUSE -> {
            playerController.pause()
            true
        }

        KeyEvent.KEYCODE_MEDIA_STOP -> {
            playerController.stopPlayback()
            true
        }

        KeyEvent.KEYCODE_MEDIA_FAST_FORWARD -> {
            playerController.seekTo((playerController.snapshot.value.positionSeconds ?: 0.0) + 30.0)
            true
        }

        KeyEvent.KEYCODE_MEDIA_REWIND -> {
            playerController.seekTo((playerController.snapshot.value.positionSeconds ?: 0.0) - 15.0)
            true
        }

        else -> false
    }

    private suspend fun bootstrapReceiver(force: Boolean) {
        if (!isForeground) {
            return
        }
        if (!force && sessionToken != null && heartbeatJob?.isActive == true && eventJob?.isActive == true) {
            return
        }

        cancelSessionLoops()

        val serverOrigin = currentPreferences.serverOrigin
        if (serverOrigin.isNullOrBlank()) {
            mutableUiState.value = ReceiverUiState(
                status = ReceiverStatus.NEEDS_SERVER_CONFIG,
                serverInput = uiState.value.serverInput,
                detailMessage = "Enter the public Euripus server URL to connect this TV.",
            )
            return
        }

        endpointConfig = runCatching { normalizeServerOrigin(serverOrigin) }.getOrElse { error ->
            mutableUiState.value = ReceiverUiState(
                status = ReceiverStatus.NEEDS_SERVER_CONFIG,
                configuredServerOrigin = serverOrigin,
                serverInput = serverOrigin,
                errorMessage = error.message,
                detailMessage = "Update the saved server URL and try again.",
            )
            return
        }

        mutableUiState.update {
            it.copy(
                status = ReceiverStatus.STARTING_SESSION,
                configuredServerOrigin = endpointConfig?.publicOrigin,
                pairingCode = null,
                errorMessage = null,
                detailMessage = "Starting the receiver session.",
                isBusy = true,
            )
        }

        runCatching {
            val deviceKey = preferencesRepository.ensureDeviceKey()
            val config = checkNotNull(endpointConfig)
            apiService.validateServer(config)
            apiService.createReceiverSession(
                config = config,
                payload = ReceiverSessionPayloadDto(
                    deviceKey = deviceKey,
                    name = "Android TV receiver",
                    platform = "android-tv",
                    formFactorHint = "tv",
                    appKind = "receiver-android-tv",
                    publicOrigin = config.publicOrigin,
                    receiverCredential = currentPreferences.receiverCredential,
                ),
            )
        }.onSuccess { session ->
            sessionToken = session.sessionToken
            session.receiverCredential?.let { credential ->
                currentPreferences = currentPreferences.copy(receiverCredential = credential)
                preferencesRepository.saveReceiverCredential(credential)
            }
            mutableUiState.update {
                it.copy(
                    status = if (session.pairingCode != null) ReceiverStatus.PAIRING else ReceiverStatus.IDLE,
                    configuredServerOrigin = endpointConfig?.publicOrigin,
                    pairingCode = session.pairingCode,
                    errorMessage = null,
                    detailMessage = if (session.pairingCode != null) {
                        "Open Euripus on your phone and enter the code below."
                    } else {
                        "Receiver is ready."
                    },
                    isBusy = false,
                )
            }
            startSessionLoops()
            syncPlaybackStateOnce(session.sessionToken)
        }.onFailure { error ->
            if (error is CancellationException) {
                throw error
            }
            mutableUiState.update {
                it.copy(
                    status = ReceiverStatus.ERROR,
                    pairingCode = null,
                    errorMessage = error.message ?: "Receiver startup failed.",
                    detailMessage = "Check the server URL and make sure Euripus is reachable.",
                    isBusy = false,
                )
            }
        }
    }

    private fun startSessionLoops() {
        val config = endpointConfig ?: return
        val token = sessionToken ?: return

        heartbeatJob = viewModelScope.launch {
            while (true) {
                try {
                    apiService.heartbeat(config, token)
                } catch (error: CancellationException) {
                    throw error
                } catch (error: Throwable) {
                    if (handleLoopFailure(token, error)) {
                        break
                    }
                }
                delay(HEARTBEAT_INTERVAL_MS)
            }
        }

        eventJob = viewModelScope.launch {
            var backoffMs = 1_000L
            while (true) {
                try {
                    eventStream.open(config, token).collectLatest { event ->
                        backoffMs = 1_000L
                        handleEvent(token, event)
                    }
                } catch (error: CancellationException) {
                    throw error
                } catch (error: Throwable) {
                    if (handleLoopFailure(token, error)) {
                        break
                    }
                }
                delay(backoffMs)
                backoffMs = min(backoffMs * 2, 30_000L)
            }
        }

        playbackSyncJob = viewModelScope.launch {
            while (true) {
                syncPlaybackStateOnce(token)
                delay(PLAYBACK_SYNC_INTERVAL_MS)
            }
        }
    }

    private suspend fun handleEvent(
        token: String,
        event: ReceiverEventPayloadDto,
    ) {
        if (sessionToken != token) {
            return
        }
        val config = endpointConfig ?: return

        when (event.eventType) {
            "playback_command" -> {
                val source = event.source ?: return
                beginPendingCommand(
                    config = config,
                    token = token,
                    commandId = event.command.id,
                    kind = PendingCommandKind.PLAYBACK_SOURCE,
                )
                setPlaybackSource(source)
                maybeCompletePendingCommand()
            }

            "transport_command" -> {
                val commandKind = when (event.command.commandType) {
                    "pause" -> PendingCommandKind.PAUSE
                    "play" -> PendingCommandKind.PLAY
                    "seek" -> PendingCommandKind.SEEK
                    "stop" -> PendingCommandKind.STOP
                    else -> null
                } ?: return
                beginPendingCommand(
                    config = config,
                    token = token,
                    commandId = event.command.id,
                    kind = commandKind,
                    seekPositionSeconds = event.positionSeconds,
                )
                when (event.command.commandType) {
                    "pause" -> playerController.pause()
                    "play" -> playerController.playFromTvRemote()
                    "seek" -> event.positionSeconds?.let(playerController::seekTo)
                    "stop" -> playerController.stopPlayback()
                }
                maybeCompletePendingCommand()
            }

            "pairing_complete" -> {
                event.receiverCredential?.let { credential ->
                    currentPreferences = currentPreferences.copy(receiverCredential = credential)
                    preferencesRepository.saveReceiverCredential(credential)
                }
                mutableUiState.update {
                    it.copy(
                        pairingCode = null,
                        status = if (playerController.currentSource.value == null) {
                            ReceiverStatus.IDLE
                        } else {
                            ReceiverStatus.PLAYING
                        },
                        detailMessage = "Receiver paired successfully.",
                        errorMessage = null,
                    )
                }
            }
        }
    }

    private fun setPlaybackSource(source: PlaybackSourceDto) {
        if (source.kind == "unsupported") {
            playerController.markUnsupported(source)
            syncUiFromPlaybackState(source = source)
            return
        }

        playerController.setSource(source)
        syncUiFromPlaybackState(source = source)
    }

    private suspend fun syncPlaybackStateOnce(token: String) {
        if (sessionToken != token) {
            return
        }
        val config = endpointConfig ?: return
        val source = playerController.currentSource.value
        val snapshot = playerController.snapshot.value

        runCatching {
            apiService.updatePlaybackState(
                config = config,
                sessionToken = token,
                payload = ReceiverPlaybackStatePayloadDto(
                    title = source?.title,
                    sourceKind = source?.kind,
                    live = source?.live,
                    catchup = source?.catchup,
                    paused = snapshot.paused,
                    buffering = snapshot.buffering,
                    positionSeconds = snapshot.positionSeconds,
                    durationSeconds = snapshot.durationSeconds,
                    errorMessage = snapshot.errorMessage,
                ),
            )
        }.onFailure { error ->
            if (error is CancellationException) {
                throw error
            }
            handleLoopFailure(token, error)
        }
    }

    private fun syncUiFromPlaybackState(source: PlaybackSourceDto? = playerController.currentSource.value) {
        val snapshot = playerController.snapshot.value
        mutableUiState.update { state ->
            val nextStatus = when {
                state.status == ReceiverStatus.NEEDS_SERVER_CONFIG -> state.status
                state.status == ReceiverStatus.STARTING_SESSION -> state.status
                state.pairingCode != null -> ReceiverStatus.PAIRING
                source == null -> ReceiverStatus.IDLE
                source.kind == "unsupported" -> ReceiverStatus.ERROR
                snapshot.errorMessage != null -> ReceiverStatus.ERROR
                else -> ReceiverStatus.PLAYING
            }
            state.copy(
                source = source,
                pairingCode = if (source != null) null else state.pairingCode,
                status = nextStatus,
                errorMessage = when {
                    source?.kind == "unsupported" -> source.unsupportedReason
                        ?: "This stream is not supported on the receiver."
                    snapshot.errorMessage != null -> snapshot.errorMessage
                    state.status == ReceiverStatus.NEEDS_SERVER_CONFIG ||
                        state.status == ReceiverStatus.STARTING_SESSION ||
                        state.pairingCode != null -> state.errorMessage
                    else -> null
                },
                detailMessage = when {
                    source?.kind == "unsupported" -> source.title
                    snapshot.errorMessage != null -> source?.title
                    nextStatus == ReceiverStatus.PLAYING && snapshot.buffering -> "Buffering stream..."
                    nextStatus == ReceiverStatus.IDLE && state.detailMessage.isNullOrBlank() -> "Receiver is ready."
                    nextStatus == ReceiverStatus.PLAYING -> null
                    else -> state.detailMessage
                },
            )
        }
    }

    private suspend fun beginPendingCommand(
        config: ReceiverEndpointConfig,
        token: String,
        commandId: String,
        kind: PendingCommandKind,
        seekPositionSeconds: Double? = null,
    ) {
        pendingCommand = PendingCommand(
            id = commandId,
            kind = kind,
            seekPositionSeconds = seekPositionSeconds,
        )
        apiService.acknowledgeCommand(
            config = config,
            sessionToken = token,
            commandId = commandId,
            payload = RemoteCommandAckDto(status = "executing"),
        )
    }

    private suspend fun maybeCompletePendingCommand() {
        val pending = pendingCommand ?: return
        val config = endpointConfig ?: return
        val token = sessionToken ?: return
        val source = playerController.currentSource.value
        val snapshot = playerController.snapshot.value

        if (source?.kind == "unsupported") {
            finishPendingCommand(
                config = config,
                token = token,
                status = "failed",
                errorMessage = source.unsupportedReason ?: "This stream is not supported on the receiver.",
            )
            return
        }

        if (snapshot.errorMessage != null) {
            finishPendingCommand(
                config = config,
                token = token,
                status = "failed",
                errorMessage = snapshot.errorMessage,
            )
            return
        }

        when (pending.kind) {
            PendingCommandKind.PLAYBACK_SOURCE -> {
                if (playerController.isReadyForPlayback()) {
                    finishPendingCommand(config, token, "succeeded")
                }
            }

            PendingCommandKind.PLAY -> {
                if (playerController.isReadyForPlayback() && !snapshot.paused) {
                    finishPendingCommand(config, token, "succeeded")
                }
            }

            PendingCommandKind.PAUSE -> {
                if (source != null && snapshot.paused && !snapshot.buffering) {
                    finishPendingCommand(config, token, "succeeded")
                }
            }

            PendingCommandKind.SEEK -> {
                if (PlaybackStateHeuristics.seekCompleted(
                        targetSeconds = pending.seekPositionSeconds,
                        positionSeconds = snapshot.positionSeconds,
                        buffering = snapshot.buffering,
                        toleranceSeconds = SEEK_COMPLETION_TOLERANCE_SECONDS,
                    )
                ) {
                    finishPendingCommand(config, token, "succeeded")
                }
            }

            PendingCommandKind.STOP -> {
                if (source == null) {
                    finishPendingCommand(config, token, "succeeded")
                }
            }
        }
    }

    private suspend fun finishPendingCommand(
        config: ReceiverEndpointConfig,
        token: String,
        status: String,
        errorMessage: String? = null,
    ) {
        val pending = pendingCommand ?: return
        apiService.acknowledgeCommand(
            config = config,
            sessionToken = token,
            commandId = pending.id,
            payload = RemoteCommandAckDto(status = status, errorMessage = errorMessage),
        )
        pendingCommand = null
    }

    private suspend fun handleSessionExpired(token: String) {
        if (sessionToken != token) {
            return
        }
        cancelSessionLoops()
        sessionToken = null
        pendingCommand = null
        currentPreferences = currentPreferences.copy(receiverCredential = null)
        preferencesRepository.saveReceiverCredential(null)
        playerController.stopPlayback()
        mutableUiState.update {
            it.copy(
                source = null,
                pairingCode = null,
                status = ReceiverStatus.STARTING_SESSION,
                errorMessage = null,
                detailMessage = "Receiver authorization expired. Starting a new pairing session.",
            )
        }
        requestBootstrap(force = true)
    }

    private suspend fun handleLoopFailure(
        token: String,
        error: Throwable,
    ): Boolean {
        if (sessionToken != token) {
            return true
        }
        Log.w(TAG, "Receiver loop failure", error)
        return when (error) {
            is ReceiverAuthExpiredException -> {
                handleSessionExpired(token)
                true
            }

            is ReceiverApiException -> {
                if (error.statusCode == 401 || error.statusCode == 403) {
                    handleSessionExpired(token)
                    true
                } else {
                    mutableUiState.update {
                        it.copy(detailMessage = "Connection interrupted. Reconnecting...")
                    }
                    false
                }
            }

            else -> {
                mutableUiState.update {
                    it.copy(detailMessage = "Connection interrupted. Reconnecting...")
                }
                false
            }
        }
    }

    private fun cancelSessionLoops() {
        heartbeatJob?.cancel()
        eventJob?.cancel()
        playbackSyncJob?.cancel()
        pendingCommand = null
        heartbeatJob = null
        eventJob = null
        playbackSyncJob = null
    }

    override fun onCleared() {
        bootstrapJob?.cancel()
        cancelSessionLoops()
        playerController.release()
        super.onCleared()
    }
}
