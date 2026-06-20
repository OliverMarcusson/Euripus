package se.olivermarcusson.euripus.receiver.ui.theme

import androidx.compose.runtime.Composable
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.darkColorScheme

private val EuripusDarkColors = darkColorScheme(
    background = Obsidian,
    surface = ObsidianCard,
    primary = NeonPurple,
    onPrimary = Obsidian,
    onBackground = Lavender,
    onSurface = Lavender,
)

@Composable
fun EuripusReceiverTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = EuripusDarkColors,
        content = content,
    )
}
