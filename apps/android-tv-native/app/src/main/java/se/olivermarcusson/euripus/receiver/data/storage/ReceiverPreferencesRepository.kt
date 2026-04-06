package se.olivermarcusson.euripus.receiver.data.storage

import android.content.Context
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.io.IOException
import java.util.UUID
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map

private val Context.receiverPreferencesDataStore by preferencesDataStore(name = "receiver_preferences")

class ReceiverPreferencesRepository(
    private val context: Context,
) {
    private object Keys {
        val serverOrigin = stringPreferencesKey("server_origin")
        val deviceKey = stringPreferencesKey("device_key")
        val receiverCredential = stringPreferencesKey("receiver_credential")
    }

    val preferences: Flow<ReceiverPreferences> = context.receiverPreferencesDataStore.data
        .catch { error ->
            if (error is IOException) {
                emit(emptyPreferences())
            } else {
                throw error
            }
        }
        .map(::mapPreferences)

    suspend fun snapshot(): ReceiverPreferences = preferences.first()

    suspend fun ensureDeviceKey(): String {
        val existing = snapshot().deviceKey
        if (!existing.isNullOrBlank()) {
            return existing
        }
        val next = UUID.randomUUID().toString()
        context.receiverPreferencesDataStore.edit { prefs ->
            prefs[Keys.deviceKey] = next
        }
        return next
    }

    suspend fun saveServerOrigin(origin: String) {
        context.receiverPreferencesDataStore.edit { prefs ->
            prefs[Keys.serverOrigin] = origin
        }
    }

    suspend fun saveReceiverCredential(credential: String?) {
        context.receiverPreferencesDataStore.edit { prefs ->
            if (credential.isNullOrBlank()) {
                prefs.remove(Keys.receiverCredential)
            } else {
                prefs[Keys.receiverCredential] = credential
            }
        }
    }

    private fun mapPreferences(prefs: Preferences) = ReceiverPreferences(
        serverOrigin = prefs[Keys.serverOrigin],
        deviceKey = prefs[Keys.deviceKey],
        receiverCredential = prefs[Keys.receiverCredential],
    )
}
