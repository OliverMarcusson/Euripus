package se.olivermarcusson.euripus.receiver.config

import java.net.URI
import java.net.URISyntaxException

data class ReceiverEndpointConfig(
    val publicOrigin: String,
    val apiBaseUrl: String,
)

fun normalizeServerOrigin(raw: String): ReceiverEndpointConfig {
    val input = raw.trim()
    require(input.isNotEmpty()) { "Enter the Euripus server URL." }

    val normalizedInput = if ("://" in input) input else "https://$input"
    val parsed = try {
        URI(normalizedInput)
    } catch (_: URISyntaxException) {
        throw IllegalArgumentException("The server URL must include a valid host name.")
    }
    val scheme = parsed.scheme?.lowercase()
    require(scheme == "https" || scheme == "http") {
        "Use an http or https Euripus server URL."
    }
    require(!parsed.host.isNullOrBlank()) {
        "The server URL must include a host name."
    }

    val normalizedPath = parsed.path.orEmpty().trimEnd('/')
    val publicPath = when {
        normalizedPath.isEmpty() -> ""
        normalizedPath.endsWith("/api") -> normalizedPath.removeSuffix("/api")
        else -> normalizedPath
    }
    val publicOrigin = URI(
        scheme,
        parsed.userInfo,
        parsed.host,
        parsed.port,
        publicPath.ifEmpty { null },
        null,
        null,
    ).toString().trimEnd('/')

    return ReceiverEndpointConfig(
        publicOrigin = publicOrigin,
        apiBaseUrl = "$publicOrigin/api",
    )
}
