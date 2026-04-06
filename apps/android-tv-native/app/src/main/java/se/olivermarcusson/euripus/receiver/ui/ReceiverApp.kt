package se.olivermarcusson.euripus.receiver.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import se.olivermarcusson.euripus.receiver.session.ReceiverViewModel
import se.olivermarcusson.euripus.receiver.ui.screens.ReceiverRoot
import se.olivermarcusson.euripus.receiver.ui.theme.EuripusReceiverTheme

@Composable
fun ReceiverApp(viewModel: ReceiverViewModel) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()

    EuripusReceiverTheme {
        ReceiverRoot(state = state, viewModel = viewModel)
    }
}
