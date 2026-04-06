package se.olivermarcusson.euripus.receiver.session

import android.app.Application
import android.util.Log
import android.view.KeyEvent
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlin.math.min
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
import se.olivermarcusson.euripus.receiver.data.player.ReceiverPlayerController
import se.olivermarcusson.euripus.receiver.data.storage.ReceiverPreferences
import se.olivermarcusson.euripus.receiver.data.storage.ReceiverPreferencesRepository
import se.olivermarcusson.euripus.receiver.domain.ReceiverStatus
import se.olivermarcusson.euripus.receiver.domain.ReceiverUiState

private const val TAG = "ReceiverViewModel"
private const val HEARTBEAT_INTERVAL_MS = 15_000L
private const val PLAYBACK_SYNC_INTERVAL_MS = 1_000L

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
    private var heartbeatJob: Job? = null
    private var eventJob: Job? = null
    private var playbackSyncJob: Job? = null

    init {
        viewModelScope.launch {
            preferencesRepository.preferences.collectLatest { prefs ->
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
                if (isForeground) {
                    bootstrapReceiver(force = true)
                }
            }
        }

        viewModelScope.launch {
            playerController.currentSource.collectLatest { source ->
                mutableUiState.update { state ->
                    state.copy(
                        source = source,
                        status = when {
                            state.status == ReceiverStatus.NEEDS_SERVER_CONFIG -> state.status
                            state.pairingCode != null -> ReceiverStatus.PAIRING
                            source == null -> ReceiverStatus.IDLE
                            source.kind == "unsupported" -> ReceiverStatus.ERROR
                            else -> ReceiverStatus.PLAYING
                        },
                    )
                }
            }
        }
    }

    fun onForegroundChanged(isForeground: Boolean) {
        this.isForeground = isForeground
        if (isForeground) {
            viewModelScope.launch { bootstrapReceiver(force = false) }
        } else {
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
        viewModelScope.launch { bootstrapReceiver(force = true) }
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
                    receiverCredential = currentPreferences.receiverCredential,
                ),
            )
        }.onSuccess { session ->
            sessionToken = session.sessionToken
            currentPreferences = currentPreferences.copy(receiverCredential = session.receiverCredential)
            preferencesRepository.saveReceiverCredential(session.receiverCredential)
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
            syncPlaybackStateOnce()
        }.onFailure { error ->
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
                } catch (error: Throwable) {
                    if (handleLoopFailure(error)) {
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
                        handleEvent(event)
                    }
                } catch (error: Throwable) {
                    if (handleLoopFailure(error)) {
                        break
                    }
                }
                delay(backoffMs)
                backoffMs = min(backoffMs * 2, 30_000L)
            }
        }

        playbackSyncJob = viewModelScope.launch {
            while (true) {
                syncPlaybackStateOnce()
                delay(PLAYBACK_SYNC_INTERVAL_MS)
            }
        }
    }

    private suspend fun handleEvent(event: ReceiverEventPayloadDto) {
        val config = endpointConfig ?: return
        val token = sessionToken ?: return

        when (event.eventType) {
            "playback_command" -> {
                event.source?.let(::setPlaybackSource)
                apiService.acknowledgeCommand(
                    config = config,
                    sessionToken = token,
                    commandId = event.command.id,
                    payload = RemoteCommandAckDto(status = "acknowledged"),
                )
            }

            "transport_command" -> {
                when (event.command.commandType) {
                    "pause" -> playerController.pause()
                    "play" -> playerController.play()
                    "seek" -> event.positionSeconds?.let(playerController::seekTo)
                    "stop" -> playerController.stopPlayback()
                }
                apiService.acknowledgeCommand(
                    config = config,
                    sessionToken = token,
                    commandId = event.command.id,
                    payload = RemoteCommandAckDto(status = "acknowledged"),
                )
            }

            "pairing_complete" -> {
                currentPreferences = currentPreferences.copy(receiverCredential = event.receiverCredential)
                preferencesRepository.saveReceiverCredential(event.receiverCredential)
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
            mutableUiState.update {
                it.copy(
                    source = source,
                    status = ReceiverStatus.ERROR,
                    pairingCode = null,
                    errorMessage = source.unsupportedReason
                        ?: "This stream is not supported on the receiver.",
                    detailMessage = source.title,
                )
            }
            return
        }

        playerController.setSource(source)
        mutableUiState.update {
            it.copy(
                source = source,
                pairingCode = null,
                status = ReceiverStatus.PLAYING,
                errorMessage = null,
                detailMessage = null,
            )
        }
    }

    private suspend fun syncPlaybackStateOnce() {
        val config = endpointConfig ?: return
        val token = sessionToken ?: return
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
                    positionSeconds = snapshot.positionSeconds,
                    durationSeconds = snapshot.durationSeconds,
                ),
            )
        }.onFailure { error ->
            handleLoopFailure(error)
        }
    }

    private suspend fun handleSessionExpired() {
        cancelSessionLoops()
        sessionToken = null
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
        bootstrapReceiver(force = true)
    }

    private suspend fun handleLoopFailure(error: Throwable): Boolean {
        Log.w(TAG, "Receiver loop failure", error)
        return when (error) {
            is ReceiverAuthExpiredException -> {
                handleSessionExpired()
                true
            }

            is ReceiverApiException -> {
                if (error.statusCode == 401 || error.statusCode == 403) {
                    handleSessionExpired()
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
        heartbeatJob = null
        eventJob = null
        playbackSyncJob = null
    }

    override fun onCleared() {
        cancelSessionLoops()
        playerController.release()
        super.onCleared()
    }
}
