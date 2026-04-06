package se.olivermarcusson.euripus.receiver.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.Player
import androidx.media3.ui.PlayerView
import se.olivermarcusson.euripus.receiver.domain.ReceiverStatus
import se.olivermarcusson.euripus.receiver.domain.ReceiverUiState
import se.olivermarcusson.euripus.receiver.session.ReceiverViewModel
import se.olivermarcusson.euripus.receiver.ui.theme.Lavender
import se.olivermarcusson.euripus.receiver.ui.theme.MutedText
import se.olivermarcusson.euripus.receiver.ui.theme.NeonPurple
import se.olivermarcusson.euripus.receiver.ui.theme.NeonPurpleDeep
import se.olivermarcusson.euripus.receiver.ui.theme.Obsidian
import se.olivermarcusson.euripus.receiver.ui.theme.ObsidianCard

@Composable
fun ReceiverRoot(
    state: ReceiverUiState,
    viewModel: ReceiverViewModel,
) {
    Backdrop {
        when (state.status) {
            ReceiverStatus.NEEDS_SERVER_CONFIG -> ServerSetupScreen(
                state = state,
                onUrlChange = viewModel::onServerInputChanged,
                onConnect = viewModel::saveServerAndConnect,
            )

            ReceiverStatus.STARTING_SESSION -> LoadingScreen(state.detailMessage ?: "Starting receiver session...")
            ReceiverStatus.PAIRING -> PairingScreen(
                pairingCode = state.pairingCode.orEmpty(),
                errorMessage = state.errorMessage,
                detailMessage = state.detailMessage,
                onRefreshCode = viewModel::refreshPairingCode,
            )

            ReceiverStatus.IDLE -> IdleScreen(
                title = "Nothing is playing",
                detail = state.detailMessage ?: "Choose a channel or program from Euripus to start playback on this screen.",
            )

            ReceiverStatus.PLAYING -> PlaybackScreen(player = viewModel.player)
            ReceiverStatus.ERROR -> {
                if (state.source?.kind == "unsupported") {
                    UnsupportedScreen(
                        title = state.source.title,
                        message = state.errorMessage ?: "This stream is not supported on the receiver.",
                    )
                } else {
                    ErrorScreen(
                        message = state.errorMessage ?: "Receiver error",
                        detail = state.detailMessage,
                        onRetry = viewModel::retry,
                    )
                }
            }
        }
    }
}

@Composable
private fun Backdrop(content: @Composable () -> Unit) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.radialGradient(
                    colors = listOf(Color(0x553F0E79), Color.Transparent),
                    radius = 1000f,
                ),
            ),
            contentAlignment = Alignment.Center,
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(
                    Brush.verticalGradient(
                        listOf(Color(0xCC0A0A12), Color(0xFF05050A)),
                    ),
                ),
        )
        content()
    }
}

@Composable
private fun ServerSetupScreen(
    state: ReceiverUiState,
    onUrlChange: (String) -> Unit,
    onConnect: () -> Unit,
) {
    CenterCard {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Eyebrow("Euripus Receiver")
            Title("Connect this TV")
            Body("Enter the public Euripus server URL. The native receiver will reuse the same /api receiver protocol as the web receiver.")
            OutlinedTextField(
                value = state.serverInput,
                onValueChange = onUrlChange,
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Server URL") },
                singleLine = true,
            )
            Button(onClick = onConnect, enabled = !state.isBusy) {
                Text(if (state.isBusy) "Connecting..." else "Connect")
            }
            state.errorMessage?.let {
                MessagePanel(title = "Connection issue", message = it)
            }
        }
    }
}

@Composable
private fun LoadingScreen(message: String) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
        modifier = Modifier.fillMaxSize(),
    ) {
        CircularProgressIndicator(color = NeonPurple)
        Spacer(modifier = Modifier.height(18.dp))
        Body(message)
    }
}

@Composable
private fun PairingScreen(
    pairingCode: String,
    errorMessage: String?,
    detailMessage: String?,
    onRefreshCode: () -> Unit,
) {
    CenterCard {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(24.dp),
        ) {
            Eyebrow("Euripus Receiver")
            Title("Pair this screen")
            Body(detailMessage ?: "Open Euripus on your phone and enter the code below.")
            Box(
                modifier = Modifier
                    .clip(RoundedCornerShape(28.dp))
                    .background(Color.White.copy(alpha = 0.05f))
                    .border(1.dp, Color.White.copy(alpha = 0.08f), RoundedCornerShape(28.dp))
                    .padding(horizontal = 42.dp, vertical = 28.dp),
            ) {
                Text(
                    pairingCode.chunked(1).joinToString(" "),
                    color = Lavender,
                    fontSize = 80.sp,
                    letterSpacing = 4.sp,
                    fontWeight = FontWeight.SemiBold,
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                Button(onClick = onRefreshCode) {
                    Text("Refresh code")
                }
            }
            errorMessage?.let { MessagePanel(title = "Receiver issue", message = it) }
        }
    }
}

@Composable
private fun IdleScreen(title: String, detail: String) {
    Column(
        modifier = Modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Box(
            modifier = Modifier
                .size(92.dp)
                .clip(RoundedCornerShape(24.dp))
                .background(Color.White.copy(alpha = 0.04f)),
            contentAlignment = Alignment.Center,
        ) {
            Text("TV", color = NeonPurple, fontWeight = FontWeight.Bold, fontSize = 28.sp)
        }
        Spacer(modifier = Modifier.height(18.dp))
        Title(title)
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            detail,
            color = MutedText,
            textAlign = TextAlign.Center,
            fontSize = 20.sp,
            lineHeight = 28.sp,
            modifier = Modifier.padding(horizontal = 120.dp),
        )
    }
}

@Composable
private fun PlaybackScreen(player: Player) {
    Box(modifier = Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { context ->
                PlayerView(context).apply {
                    useController = false
                    this.player = player
                    setShutterBackgroundColor(android.graphics.Color.BLACK)
                }
            },
            update = { view ->
                view.player = player
            },
        )
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(220.dp)
                .align(Alignment.TopCenter)
                .background(
                    Brush.verticalGradient(
                        listOf(Color(0x440A0A12), Color.Transparent),
                    ),
                ),
        )
    }
}

@Composable
private fun UnsupportedScreen(title: String, message: String) {
    CenterCard(widthFraction = 0.58f) {
        MessagePanel(title = title, message = message, accent = Color(0xFFFFC98E))
    }
}

@Composable
private fun ErrorScreen(message: String, detail: String?, onRetry: () -> Unit) {
    CenterCard(widthFraction = 0.58f) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(18.dp),
        ) {
            MessagePanel(title = "Receiver error", message = message)
            if (!detail.isNullOrBlank()) {
                Body(detail)
            }
            Button(onClick = onRetry) {
                Text("Retry")
            }
        }
    }
}

@Composable
private fun CenterCard(
    widthFraction: Float = 0.66f,
    content: @Composable () -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth(widthFraction),
        colors = CardDefaults.cardColors(containerColor = ObsidianCard),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 36.dp, vertical = 34.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            content()
        }
    }
}

@Composable
private fun Eyebrow(text: String) {
    Text(
        text = text.uppercase(),
        color = Color.White.copy(alpha = 0.76f),
        fontSize = 14.sp,
        letterSpacing = 3.sp,
        fontWeight = FontWeight.Medium,
    )
}

@Composable
private fun Title(text: String) {
    Text(
        text = text,
        color = Lavender,
        fontSize = 42.sp,
        fontWeight = FontWeight.SemiBold,
        textAlign = TextAlign.Center,
    )
}

@Composable
private fun Body(text: String) {
    Text(
        text = text,
        color = MutedText,
        fontSize = 20.sp,
        lineHeight = 28.sp,
        textAlign = TextAlign.Center,
    )
}

@Composable
private fun MessagePanel(
    title: String,
    message: String,
    accent: Color = NeonPurpleDeep,
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(24.dp))
            .background(accent.copy(alpha = 0.12f))
            .border(1.dp, accent.copy(alpha = 0.32f), RoundedCornerShape(24.dp))
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text(title, color = Lavender, fontSize = 24.sp, fontWeight = FontWeight.SemiBold)
        Text(message, color = MutedText, fontSize = 19.sp, lineHeight = 27.sp)
    }
}
