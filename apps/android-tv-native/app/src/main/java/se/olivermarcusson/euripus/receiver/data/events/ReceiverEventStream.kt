package se.olivermarcusson.euripus.receiver.data.events

import java.io.IOException
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.serialization.json.Json
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.Request
import okhttp3.Response
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources
import se.olivermarcusson.euripus.receiver.config.ReceiverEndpointConfig
import se.olivermarcusson.euripus.receiver.data.api.ReceiverApiService
import se.olivermarcusson.euripus.receiver.data.api.ReceiverEventPayloadDto

class ReceiverAuthExpiredException : IOException("Receiver session is no longer authorized.")

class ReceiverEventStream(
    private val apiService: ReceiverApiService,
    private val json: Json = Json {
        ignoreUnknownKeys = true
        explicitNulls = false
    },
) {
    private val sseClient = apiService.client.newBuilder()
        // SSE is intentionally long-lived and mostly quiet between commands.
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .build()

    fun open(
        config: ReceiverEndpointConfig,
        sessionToken: String,
    ): Flow<ReceiverEventPayloadDto> = callbackFlow {
        val request = Request.Builder()
            .url("${config.apiBaseUrl}/receiver/events?sessionToken=$sessionToken".toHttpUrl())
            .get()
            .build()

        val eventSource = EventSources.createFactory(sseClient).newEventSource(
            request,
            object : EventSourceListener() {
                override fun onEvent(
                    eventSource: EventSource,
                    id: String?,
                    type: String?,
                    data: String,
                ) {
                    val payload = runCatching {
                        json.decodeFromString(ReceiverEventPayloadDto.serializer(), data)
                    }.getOrElse { error ->
                        close(IOException("Failed to decode receiver event payload.", error))
                        return
                    }
                    trySend(payload)
                }

                override fun onFailure(
                    eventSource: EventSource,
                    t: Throwable?,
                    response: Response?,
                ) {
                    when (response?.code) {
                        401, 403 -> close(ReceiverAuthExpiredException())
                        else -> close(t ?: IOException("Receiver event stream disconnected."))
                    }
                }
            },
        )

        awaitClose {
            eventSource.cancel()
        }
    }
}
