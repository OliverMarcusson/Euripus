package se.olivermarcusson.euripus.receiver.data.storage

data class ReceiverPreferences(
    val serverOrigin: String? = null,
    val deviceKey: String? = null,
    val receiverCredential: String? = null,
)
