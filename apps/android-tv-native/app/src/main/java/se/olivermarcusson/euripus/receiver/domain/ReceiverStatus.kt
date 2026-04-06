package se.olivermarcusson.euripus.receiver.domain

enum class ReceiverStatus {
    NEEDS_SERVER_CONFIG,
    STARTING_SESSION,
    PAIRING,
    IDLE,
    PLAYING,
    ERROR,
}
