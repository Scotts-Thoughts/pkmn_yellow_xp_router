import logging

from PySide6.QtCore import QObject, QTimer

from pkmn.gen_factory import current_gen_info
from route_recording.recorder import RecorderController, RecorderGameHookClient
from utils.constants import const

logger = logging.getLogger(__name__)


class RecorderStatus(QObject):
    """Manages the GameHook connection lifecycle and updates external status widgets."""

    def __init__(self, main_controller, recorder_controller, client_status_label, reconnect_button, parent=None):
        super().__init__(parent)
        self._main_controller = main_controller
        self._recorder_controller: RecorderController = recorder_controller
        self._gamehook_client: RecorderGameHookClient = None

        # External widgets (owned by the status bar) that we update.
        self.client_status_label = client_status_label
        self.connection_retry_button = reconnect_button
        self.connection_retry_button.clicked.connect(self.reconnect_button_pressed)

        # Register callbacks with the controllers.
        # Recorder callbacks may fire from background GameHook threads, so we
        # wrap them with QTimer.singleShot(0, ...) to marshal onto the main
        # (GUI) thread before touching any widgets.
        self._unregister_record_mode = self._main_controller.register_record_mode_change(
            lambda: QTimer.singleShot(0, self.on_recording_mode_changed)
        )
        self._unregister_status = self._recorder_controller.register_recorder_status_change(
            lambda: QTimer.singleShot(0, self.on_recording_status_changed)
        )
        self._unregister_ready = self._recorder_controller.register_recorder_ready_change(
            lambda: QTimer.singleShot(0, self.on_recording_ready_changed)
        )
        self._unregister_game_state = self._recorder_controller.register_recorder_game_state_change(
            lambda: QTimer.singleShot(0, self.on_recording_game_state_changed)
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
