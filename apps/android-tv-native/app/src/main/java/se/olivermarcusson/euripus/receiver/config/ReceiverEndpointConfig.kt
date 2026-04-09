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

    val normalizedInput = if ("://" in input) {
        input
    } else {
        "${defaultSchemeForInput(input)}://$input"
    }
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

private fun defaultSchemeForInput(input: String): String {
    val hostCandidate = input.substringBefore('/').substringBefore(':').trim()
    return if (isLikelyLocalNetworkHost(hostCandidate)) "http" else "https"
}

private fun isLikelyLocalNetworkHost(host: String): Boolean {
    if (host.isBlank()) {
        return false
    }

    val lowercaseHost = host.lowercase()
    if (
        lowercaseHost == "localhost" ||
        lowercaseHost.endsWith(".local") ||
        lowercaseHost == "10.0.2.2"
    ) {
        return true
    }

    val segments = lowercaseHost.split('.')
    if (segments.size != 4 || segments.any { it.toIntOrNull() == null }) {
        return false
    }

    val octets = segments.map { it.toInt() }
    return when {
        octets[0] == 10 -> true
        octets[0] == 127 -> true
        octets[0] == 192 && octets[1] == 168 -> true
        octets[0] == 172 && octets[1] in 16..31 -> true
        else -> false
    }
}
