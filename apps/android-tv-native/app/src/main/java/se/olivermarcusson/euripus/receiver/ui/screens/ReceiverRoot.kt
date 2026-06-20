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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import android.view.LayoutInflater
import androidx.media3.common.Player
import androidx.media3.ui.PlayerView
import androidx.tv.material3.Button
import androidx.tv.material3.Surface
import androidx.tv.material3.SurfaceDefaults
import androidx.tv.material3.Text
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import se.olivermarcusson.euripus.receiver.R
import se.olivermarcusson.euripus.receiver.data.api.ProgramDto
import se.olivermarcusson.euripus.receiver.data.api.ReceiverFavoriteChannelEntryDto
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

            ReceiverStatus.PLAYING -> PlaybackScreen(
                player = viewModel.player,
                state = state,
            )
            ReceiverStatus.ERROR -> {
                if (state.source?.kind == "unsupported") {
                    UnsupportedScreen(
                        title = state.source.title,
                        message = state.errorMessage ?: "This stream is not supported on the receiver.",
                    )
                } else if (state.source != null && !state.errorMessage.isNullOrBlank()) {
                    UnsupportedScreen(
                        title = state.source.title,
                        message = state.errorMessage,
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
    val connectFocusRequester = remember { FocusRequester() }

    CenterCard {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Eyebrow("Euripus Receiver")
            Title("Connect this TV")
            Body("Enter your Euripus server URL. For local testing you can use a LAN address like http://192.168.1.42:5173, and the receiver will connect to the same /api receiver endpoints.")
            ServerUrlField(
                value = state.serverInput,
                onValueChange = onUrlChange,
                onMoveToConnect = { connectFocusRequester.requestFocus() },
                onConnect = onConnect,
            )
            Button(
                onClick = onConnect,
                enabled = !state.isBusy,
                modifier = Modifier.focusRequester(connectFocusRequester),
            ) {
                Text(if (state.isBusy) "Connecting..." else "Connect")
            }
            state.errorMessage?.let {
                MessagePanel(title = "Connection issue", message = it)
            }
        }
    }
}

@Composable
private fun ServerUrlField(
    value: String,
    onValueChange: (String) -> Unit,
    onMoveToConnect: () -> Unit,
    onConnect: () -> Unit,
) {
    BasicTextField(
        value = value,
        onValueChange = onValueChange,
        singleLine = true,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { onConnect() }),
        textStyle = androidx.compose.ui.text.TextStyle(
            color = Lavender,
            fontSize = 20.sp,
        ),
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(Color.White.copy(alpha = 0.05f))
            .border(1.dp, Color.White.copy(alpha = 0.18f), RoundedCornerShape(16.dp))
            .onPreviewKeyEvent { event ->
                if (event.type == KeyEventType.KeyDown && event.key == Key.DirectionDown) {
                    onMoveToConnect()
                    true
                } else {
                    false
                }
            }
            .padding(horizontal = 18.dp, vertical = 16.dp),
        decorationBox = { innerTextField ->
            Box(contentAlignment = Alignment.CenterStart) {
                if (value.isBlank()) {
                    Text("Server URL", color = MutedText, fontSize = 20.sp)
                }
                innerTextField()
            }
        },
    )
}

@Composable
private fun LoadingScreen(message: String) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
        modifier = Modifier.fillMaxSize(),
    ) {
        LoadingMark()
        Spacer(modifier = Modifier.height(18.dp))
        Body(message)
    }
}

@Composable
private fun LoadingMark() {
    Box(
        modifier = Modifier
            .size(54.dp)
            .clip(RoundedCornerShape(18.dp))
            .background(NeonPurple.copy(alpha = 0.14f))
            .border(2.dp, NeonPurple.copy(alpha = 0.72f), RoundedCornerShape(18.dp)),
        contentAlignment = Alignment.Center,
    ) {
        Text("...", color = Lavender, fontSize = 22.sp, fontWeight = FontWeight.SemiBold)
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
private fun PlaybackScreen(
    player: Player,
    state: ReceiverUiState,
) {
    Box(modifier = Modifier.fillMaxSize().background(Color.Black)) {
        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { context ->
                (LayoutInflater.from(context).inflate(R.layout.receiver_player_view, null, false) as PlayerView).apply {
                    useController = false
                    this.player = player
                    setShutterBackgroundColor(android.graphics.Color.BLACK)
                    setKeepContentOnPlayerReset(false)
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
        if (state.channelViewerOpen) {
            ChannelViewerOverlay(state = state)
        }
    }
}

@Composable
private fun ChannelViewerOverlay(state: ReceiverUiState) {
    val listState = rememberLazyListState()
    LaunchedEffect(state.selectedChannelIndex, state.favoriteChannels.size) {
        if (state.favoriteChannels.isNotEmpty()) {
            listState.animateScrollToItem(state.selectedChannelIndex)
        }
    }
    val selectedEntry = state.favoriteChannels.getOrNull(state.selectedChannelIndex)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black.copy(alpha = 0.34f)),
        contentAlignment = Alignment.BottomCenter,
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth()
                .height(360.dp)
                .padding(horizontal = 36.dp, vertical = 28.dp),
            colors = SurfaceDefaults.colors(containerColor = Color(0xEE111017)),
            shape = RoundedCornerShape(12.dp),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(18.dp),
                horizontalArrangement = Arrangement.spacedBy(18.dp),
            ) {
                Column(modifier = Modifier.width(430.dp)) {
                    Text(
                        "Favorite channels",
                        color = Lavender,
                        fontSize = 20.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                    Spacer(modifier = Modifier.height(12.dp))
                    when {
                        state.channelViewerLoading -> ChannelViewerMessage("Loading channels...")
                        state.favoriteChannels.isEmpty() -> ChannelViewerMessage("No favorite channels yet")
                        else -> LazyColumn(
                            state = listState,
                            verticalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            itemsIndexed(state.favoriteChannels) { index, entry ->
                                ChannelListItem(
                                    entry = entry,
                                    selected = index == state.selectedChannelIndex,
                                    tuning = state.tuningChannelId == entry.channel.id,
                                )
                            }
                        }
                    }
                }
                EpgPanel(
                    entry = selectedEntry,
                    error = state.channelViewerError,
                    loading = state.channelViewerLoading,
                )
            }
        }
    }
}

@Composable
private fun ChannelViewerMessage(message: String) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(190.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(message, color = MutedText, fontSize = 20.sp, textAlign = TextAlign.Center)
    }
}

@Composable
private fun ChannelListItem(
    entry: ReceiverFavoriteChannelEntryDto,
    selected: Boolean,
    tuning: Boolean,
) {
    val borderColor = if (selected) NeonPurple else Color.White.copy(alpha = 0.08f)
    val backgroundColor = if (selected) NeonPurple.copy(alpha = 0.2f) else Color.White.copy(alpha = 0.05f)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(backgroundColor)
            .border(1.dp, borderColor, RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            modifier = Modifier
                .size(44.dp)
                .clip(RoundedCornerShape(8.dp))
                .background(Color.White.copy(alpha = 0.08f)),
            contentAlignment = Alignment.Center,
        ) {
            Text(channelInitials(entry.channel.name), color = Lavender, fontSize = 15.sp, fontWeight = FontWeight.Bold)
        }
        Column(modifier = Modifier.weight(1f)) {
            Text(
                entry.channel.name,
                color = Lavender,
                fontSize = 18.sp,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                entry.program?.title ?: entry.channel.categoryName ?: "No guide data",
                color = MutedText,
                fontSize = 14.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        if (tuning) {
            Text("Tuning", color = NeonPurple, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
        }
    }
}

@Composable
private fun EpgPanel(
    entry: ReceiverFavoriteChannelEntryDto?,
    error: String?,
    loading: Boolean,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .clip(RoundedCornerShape(8.dp))
            .background(Color.White.copy(alpha = 0.04f))
            .border(1.dp, Color.White.copy(alpha = 0.08f), RoundedCornerShape(8.dp))
            .padding(18.dp),
    ) {
        if (!error.isNullOrBlank()) {
            MessagePanel(title = "Channel viewer", message = error)
            return@Column
        }
        if (loading && entry == null) {
            ChannelViewerMessage("Loading guide...")
            return@Column
        }
        if (entry == null) {
            ChannelViewerMessage("Pick a favorite channel to see its guide")
            return@Column
        }

        Text(
            entry.channel.name,
            color = Lavender,
            fontSize = 28.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            entry.channel.categoryName ?: "Favorite channel",
            color = MutedText,
            fontSize = 15.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(modifier = Modifier.height(18.dp))
        ProgramBlock(label = "Now", program = entry.program)
        Spacer(modifier = Modifier.height(14.dp))
        Text("Upcoming", color = Lavender, fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        Spacer(modifier = Modifier.height(8.dp))
        if (entry.upcomingPrograms.isEmpty()) {
            Text("No upcoming listings", color = MutedText, fontSize = 16.sp)
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                entry.upcomingPrograms.take(4).forEach { program ->
                    ProgramRow(program)
                }
            }
        }
    }
}

@Composable
private fun ProgramBlock(label: String, program: ProgramDto?) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(NeonPurple.copy(alpha = 0.12f))
            .border(1.dp, NeonPurple.copy(alpha = 0.22f), RoundedCornerShape(8.dp))
            .padding(14.dp),
    ) {
        Text(label, color = NeonPurple, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        Text(
            program?.title ?: "No current listing",
            color = Lavender,
            fontSize = 22.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (program != null) {
            Text(programTimeRange(program), color = MutedText, fontSize = 15.sp)
        }
    }
}

@Composable
private fun ProgramRow(program: ProgramDto) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(programTimeRange(program), color = NeonPurple, fontSize = 14.sp, modifier = Modifier.width(92.dp))
        Text(
            program.title,
            color = Lavender,
            fontSize = 16.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
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
    Surface(
        modifier = Modifier.fillMaxWidth(widthFraction),
        colors = SurfaceDefaults.colors(containerColor = ObsidianCard),
        shape = RoundedCornerShape(12.dp),
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

private fun channelInitials(name: String): String =
    name.split(Regex("\\s+"))
        .mapNotNull { part -> part.firstOrNull()?.uppercaseChar() }
        .take(2)
        .joinToString("")
        .ifBlank { "TV" }

private fun programTimeRange(program: ProgramDto): String {
    val formatter = DateTimeFormatter.ofPattern("HH:mm").withZone(ZoneId.systemDefault())
    return runCatching {
        "${formatter.format(Instant.parse(program.startAt))} - ${formatter.format(Instant.parse(program.endAt))}"
    }.getOrElse {
        ""
    }
}
