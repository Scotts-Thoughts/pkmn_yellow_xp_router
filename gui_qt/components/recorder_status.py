import logging

from PySide6.QtCore import QObject, Signal, Slot

from pkmn.gen_factory import current_gen_info
from route_recording.recorder import RecorderController, RecorderGameHookClient
from utils.constants import const

logger = logging.getLogger(__name__)


class RecorderStatus(QObject):
    """Manages the GameHook connection lifecycle and updates external status widgets.

    GameHook / recorder callbacks fire from background threads (SignalR,
    FSM processing).  We use Qt Signals so that cross-thread emissions
    are automatically queued onto the GUI thread — QTimer.singleShot does
    NOT work reliably when called from a non-Qt thread.
    """

    # Signals used to marshal background-thread callbacks to the GUI thread.
    _sig_record_mode_changed = Signal()
    _sig_status_changed = Signal()
    _sig_ready_changed = Signal()
    _sig_game_state_changed = Signal()

    def __init__(self, main_controller, recorder_controller, client_status_label, reconnect_button, parent=None):
        super().__init__(parent)
        self._main_controller = main_controller
        self._recorder_controller: RecorderController = recorder_controller
        self._gamehook_client: RecorderGameHookClient = None

        # External widgets (owned by the status bar) that we update.
        self.client_status_label = client_status_label
        self.connection_retry_button = reconnect_button
        self.connection_retry_button.clicked.connect(self.reconnect_button_pressed)

        # Connect internal signals -> slots (queued automatically across threads).
        self._sig_record_mode_changed.connect(self.on_recording_mode_changed)
        self._sig_status_changed.connect(self.on_recording_status_changed)
        self._sig_ready_changed.connect(self.on_recording_ready_changed)
        self._sig_game_state_changed.connect(self.on_recording_game_state_changed)

        # Register callbacks with the controllers.
        # Emitting a Signal is thread-safe; the connected slot runs on the
        # GUI thread thanks to Qt's automatic queued connections.
        self._unregister_record_mode = self._main_controller.register_record_mode_change(
            self._sig_record_mode_changed.emit
        )
        self._unregister_status = self._recorder_controller.register_recorder_status_change(
            self._sig_status_changed.emit
        )
        self._unregister_ready = self._recorder_controller.register_recorder_ready_change(
            self._sig_ready_changed.emit
        )
        self._unregister_game_state = self._recorder_controller.register_recorder_game_state_change(
            self._sig_game_state_changed.emit
        )

    def on_recording_mode_changed(self):
        if self._main_controller.is_record_mode_active():
            if self._gamehook_client is not None:
                logger.warning("Recording mode set to active, but gamehook client was already active")
                return

            self.client_status_label.setText("Client Status: Connecting...")
            try:
                self._gamehook_client = current_gen_info().get_recorder_client(self._recorder_controller)
                self._gamehook_client.connect()
            except NotImplementedError:
                self.client_status_label.setText("No recorder has been created yet for the current version")
                self.connection_retry_button.setEnabled(False)
            except Exception as e:
                logger.error("General exception trying to create and connect gamehook client")
                logger.exception(e)
                self.client_status_label.setText(
                    f"Exception encountered trying to connect to gamehook: {type(e)}. Check logs for more details"
                )
        else:
            if self._gamehook_client is not None:
                self._gamehook_client.disconnect()
                self._gamehook_client = None

    def on_recording_status_changed(self):
        self.client_status_label.setText(f"Client Status: {self._recorder_controller.get_status()}")
        if self._recorder_controller.get_status() == const.RECORDING_STATUS_DISCONNECTED:
            self.connection_retry_button.setEnabled(True)

    def on_recording_game_state_changed(self):
        pass

    def on_recording_ready_changed(self):
        if self._recorder_controller.is_ready():
            self.connection_retry_button.setEnabled(False)
        else:
            self.connection_retry_button.setEnabled(True)

    def reconnect_button_pressed(self):
        if self._gamehook_client is not None:
            self._gamehook_client.connect()
