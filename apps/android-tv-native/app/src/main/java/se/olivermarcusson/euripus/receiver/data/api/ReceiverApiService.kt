package se.olivermarcusson.euripus.receiver.data.api

import android.util.Log
import java.io.IOException
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.logging.HttpLoggingInterceptor
import se.olivermarcusson.euripus.receiver.BuildConfig
import se.olivermarcusson.euripus.receiver.config.ReceiverEndpointConfig

private const val TAG = "ReceiverApiService"

class ReceiverApiException(
    message: String,
    val statusCode: Int? = null,
) : IOException(message)

class ReceiverApiService(
    private val json: Json = Json {
        ignoreUnknownKeys = true
        explicitNulls = false
    },
) {
    private val mediaType = "application/json".toMediaType()

    val client: OkHttpClient = OkHttpClient.Builder()
        .addInterceptor(
            HttpLoggingInterceptor { message -> Log.d(TAG, message) }.apply {
                level = if (BuildConfig.DEBUG) {
                    HttpLoggingInterceptor.Level.BASIC
                } else {
                    HttpLoggingInterceptor.Level.NONE
                }
            },
        )
        .build()

    suspend fun validateServer(config: ReceiverEndpointConfig) {
        execute<Unit>(
            Request.Builder()
                .url("${config.publicOrigin}/health".toHttpUrl())
                .get()
                .build(),
            allowEmptyBody = true,
        )
    }

    suspend fun createReceiverSession(
        config: ReceiverEndpointConfig,
        payload: ReceiverSessionPayloadDto,
    ): ReceiverSessionResponseDto = execute(
        Request.Builder()
            .url("${config.apiBaseUrl}/receiver/session".toHttpUrl())
            .post(
                json.encodeToString(ReceiverSessionPayloadDto.serializer(), payload)
                    .toRequestBody(mediaType),
            )
            .build(),
        ReceiverSessionResponseDto.serializer(),
    )

    suspend fun issuePairingCode(
        config: ReceiverEndpointConfig,
        sessionToken: String,
    ): ReceiverPairingCodeDto = execute(
        Request.Builder()
            .url("${config.apiBaseUrl}/receiver/pairing-code".toHttpUrl())
            .header("Authorization", "Bearer $sessionToken")
            .post("{}".toRequestBody(mediaType))
            .build(),
        ReceiverPairingCodeDto.serializer(),
    )

    suspend fun heartbeat(
        config: ReceiverEndpointConfig,
        sessionToken: String,
    ) {
        execute<Unit>(
            Request.Builder()
                .url("${config.apiBaseUrl}/receiver/heartbeat".toHttpUrl())
                .header("Authorization", "Bearer $sessionToken")
                .post("{}".toRequestBody(mediaType))
                .build(),
            allowEmptyBody = true,
        )
    }

    suspend fun updatePlaybackState(
        config: ReceiverEndpointConfig,
        sessionToken: String,
        payload: ReceiverPlaybackStatePayloadDto,
    ) {
        execute<Unit>(
            Request.Builder()
                .url("${config.apiBaseUrl}/receiver/playback-state".toHttpUrl())
                .header("Authorization", "Bearer $sessionToken")
                .post(
                    json.encodeToString(ReceiverPlaybackStatePayloadDto.serializer(), payload)
                        .toRequestBody(mediaType),
                )
                .build(),
            allowEmptyBody = true,
        )
    }

    suspend fun acknowledgeCommand(
        config: ReceiverEndpointConfig,
        sessionToken: String,
        commandId: String,
        payload: RemoteCommandAckDto,
    ) {
        execute<Unit>(
            Request.Builder()
                .url("${config.apiBaseUrl}/receiver/commands/$commandId/ack".toHttpUrl())
                .header("Authorization", "Bearer $sessionToken")
                .post(
                    json.encodeToString(RemoteCommandAckDto.serializer(), payload)
                        .toRequestBody(mediaType),
                )
                .build(),
            allowEmptyBody = true,
        )
    }

    private suspend fun <T> execute(
        request: Request,
        deserializer: kotlinx.serialization.DeserializationStrategy<T>? = null,
        allowEmptyBody: Boolean = false,
    ): T = suspendCancellableCoroutine { continuation ->
        val call = client.newCall(request)
        continuation.invokeOnCancellation {
            call.cancel()
        }
        call.enqueue(object : Callback {
            override fun onFailure(call: Call, e: IOException) {
                if (continuation.isCancelled) {
                    return
                }
                continuation.resumeWithException(e)
            }

            override fun onResponse(call: Call, response: Response) {
                if (continuation.isCancelled) {
                    response.close()
                    return
                }
                try {
                    val result = response.use { httpResponse ->
                        val bodyString = httpResponse.body?.string().orEmpty()
                        if (!httpResponse.isSuccessful) {
                            val apiError = runCatching {
                                json.decodeFromString(ApiError.serializer(), bodyString)
                            }.getOrNull()
                            throw ReceiverApiException(
                                apiError?.message ?: httpResponse.message.ifBlank { "Request failed." },
                                httpResponse.code,
                            )
                        }

                        if (deserializer == null) {
                            @Suppress("UNCHECKED_CAST")
                            Unit as T
                        } else if (bodyString.isBlank()) {
                            if (allowEmptyBody) {
                                @Suppress("UNCHECKED_CAST")
                                Unit as T
                            } else {
                                throw ReceiverApiException("The server returned an empty response.")
                            }
                        } else {
                        try {
                            json.decodeFromString(deserializer, bodyString)
                        } catch (error: SerializationException) {
                            throw ReceiverApiException(
                                "Failed to read the server response: ${error.message}",
                            )
                        }
                        }
                    }
                    continuation.resume(result)
                } catch (error: Throwable) {
                    continuation.resumeWithException(error)
                }
            }
        })
    }
}
