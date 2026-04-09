package se.olivermarcusson.euripus.receiver.config

import org.junit.Assert.assertEquals
import org.junit.Test

class ReceiverEndpointConfigTest {
    @Test
    fun `normalizes api urls back to the public origin`() {
        val config = normalizeServerOrigin("https://example.com/api")

        assertEquals("https://example.com", config.publicOrigin)
        assertEquals("https://example.com/api", config.apiBaseUrl)
    }

    @Test
    fun `adds https when the user enters only a hostname`() {
        val config = normalizeServerOrigin("tv.olivermarcusson.se")

        assertEquals("https://tv.olivermarcusson.se", config.publicOrigin)
        assertEquals("https://tv.olivermarcusson.se/api", config.apiBaseUrl)
    }

    @Test
    fun `defaults local network hosts to http for dev testing`() {
        val config = normalizeServerOrigin("192.168.1.42:5173")

        assertEquals("http://192.168.1.42:5173", config.publicOrigin)
        assertEquals("http://192.168.1.42:5173/api", config.apiBaseUrl)
    }

    @Test
    fun `defaults localhost to http for dev testing`() {
        val config = normalizeServerOrigin("localhost:5173")

        assertEquals("http://localhost:5173", config.publicOrigin)
        assertEquals("http://localhost:5173/api", config.apiBaseUrl)
    }

    @Test(expected = IllegalArgumentException::class)
    fun `rejects urls without a scheme`() {
        normalizeServerOrigin("not a valid host")
    }
}
