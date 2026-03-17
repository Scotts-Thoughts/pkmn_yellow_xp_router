import logging

from PySide6.QtWidgets import (
    QGroupBox, QWidget, QLabel, QGridLayout, QHBoxLayout, QVBoxLayout,
    QSizePolicy,
)
from PySide6.QtCore import Qt

from gui_qt.components.custom_components import (
    SimpleButton, SimpleEntry, SimpleOptionMenu, AmountEntry,
)
from pkmn.gen_factory import current_gen_info
from routing.route_events import (
    EventDefinition, InventoryEventDefinition, VitaminEventDefinition,
    RareCandyEventDefinition, HoldItemEventDefinition, LearnMoveEventDefinition,
)
from utils.constants import const

logger = logging.getLogger(__name__)


class QuickItemAdd(QGroupBox):
    def __init__(self, controller, parent=None):
        super().__init__("Items", parent)
        self._controller = controller

        self.option_menu_width = 15

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(5, 5, 5, 5)

        # --- Dropdowns / inputs area ---
        dropdowns_widget = QWidget(self)
        dropdowns_layout = QGridLayout(dropdowns_widget)
        dropdowns_layout.setContentsMargins(0, 0, 0, 0)
        dropdowns_layout.setHorizontalSpacing(2)
        dropdowns_layout.setVerticalSpacing(1)

        cur_row = 0

        # Row 0: Search + Item Type
        self._item_filter_label = QLabel("Search:", dropdowns_widget)
        dropdowns_layout.addWidget(self._item_filter_label, cur_row, 0)

        self._item_filter = SimpleEntry(callback=self.item_filter_callback, parent=dropdowns_widget)
        dropdowns_layout.addWidget(self._item_filter, cur_row, 1)

        self._item_type_label = QLabel("Item Type:", dropdowns_widget)
        dropdowns_layout.addWidget(self._item_type_label, cur_row, 2)

        self._item_type_selector = SimpleOptionMenu(
            option_list=const.ITEM_TYPES,
            callback=self.item_filter_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._item_type_selector, cur_row, 3)
        cur_row += 1

        # Row 1: Item + Mart
        self._item_selector_label = QLabel("Item:", dropdowns_widget)
        dropdowns_layout.addWidget(self._item_selector_label, cur_row, 0)

        self._item_selector = SimpleOptionMenu(
            option_list=[const.NO_ITEM],
            callback=self.item_selector_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._item_selector, cur_row, 1)

        self._item_mart_label = QLabel("Mart:", dropdowns_widget)
        dropdowns_layout.addWidget(self._item_mart_label, cur_row, 2)

        self._item_mart_selector = SimpleOptionMenu(
            option_list=[const.ITEM_TYPE_ALL_ITEMS],
            callback=self.item_filter_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._item_mart_selector, cur_row, 3)
        cur_row += 1

        # Row 2: Quantity
        self._item_amount_label = QLabel("Quantity:", dropdowns_widget)
        dropdowns_layout.addWidget(self._item_amount_label, cur_row, 0)

        self._item_amount = AmountEntry(
            min_val=1,
            callback=self.item_selector_callback,
            parent=dropdowns_widget,
        )
        dropdowns_layout.addWidget(self._item_amount, cur_row, 1)
        cur_row += 1

        # Row 3: Purchase cost + Sell cost
        self._purchase_cost_label = QLabel("Purchase:", dropdowns_widget)
        dropdowns_layout.addWidget(self._purchase_cost_label, cur_row, 0)

        self._purchase_cost_amt = QLabel("", dropdowns_widget)
        self._purchase_cost_amt.setAlignment(Qt.AlignRight | Qt.AlignVCenter)
        dropdowns_layout.addWidget(self._purchase_cost_amt, cur_row, 1)

        self._sell_cost_label = QLabel("Sell Price:", dropdowns_widget)
        dropdowns_layout.addWidget(self._sell_cost_label, cur_row, 2)

        self._sell_cost_amt = QLabel("", dropdowns_widget)
        self._sell_cost_amt.setAlignment(Qt.AlignRight | Qt.AlignVCenter)
        dropdowns_layout.addWidget(self._sell_cost_amt, cur_row, 3)
        cur_row += 1

        main_layout.addWidget(dropdowns_widget)

        # --- Buttons area ---
        buttons_widget = QWidget(self)
        buttons_layout = QHBoxLayout(buttons_widget)
        buttons_layout.setContentsMargins(0, 0, 0, 0)
        buttons_layout.setSpacing(2)

        btn_width = 60

        self._acquire_button = SimpleButton(text="Get", parent=buttons_widget, width=btn_width)
        self._acquire_button.clicked.connect(self._acquire_item)
        buttons_layout.addWidget(self._acquire_button)

        self._drop_button = SimpleButton(text="Drop", parent=buttons_widget, width=btn_width)
        self._drop_button.clicked.connect(self._drop_item)
        buttons_layout.addWidget(self._drop_button)

        buttons_layout.addStretch(1)

        self._use_button = SimpleButton(text="Use", parent=buttons_widget, width=btn_width)
        self._use_button.clicked.connect(self._use_item)
        buttons_layout.addWidget(self._use_button)

        self._hold_button = SimpleButton(text="Hold", parent=buttons_widget, width=btn_width)
        self._hold_button.clicked.connect(self._hold_item)
        buttons_layout.addWidget(self._hold_button)

        self._tm_hm_button = SimpleButton(text="TM/HM", parent=buttons_widget, width=btn_width)
        self._tm_hm_button.clicked.connect(self._learn_move)
        buttons_layout.addWidget(self._tm_hm_button)

        buttons_layout.addStretch(1)

        self._buy_button = SimpleButton(text="Buy", parent=buttons_widget, width=btn_width)
        self._buy_button.clicked.connect(self._buy_item)
        buttons_layout.addWidget(self._buy_button)

        self._sell_button = SimpleButton(text="Sell", parent=buttons_widget, width=btn_width)
        self._sell_button.clicked.connect(self._sell_item)
        buttons_layout.addWidget(self._sell_button)

        main_layout.addWidget(buttons_widget)

        # Register callbacks
        self._unregister_selection = self._controller.register_event_selection(self.update_button_status)
        self._unregister_version = self._controller.register_version_change(self.update_pkmn_version)
        self.update_button_status()

    def update_pkmn_version(self):
        self._item_selector.new_values(current_gen_info().item_db().get_filtered_names())
        self._item_mart_selector.new_values(
            [const.ITEM_TYPE_ALL_ITEMS] + sorted(list(current_gen_info().item_db().mart_items.keys()))
        )

    def update_button_status(self):
        if not self._controller.can_insert_after_current_selection():
            self._acquire_button.disable()
            self._drop_button.disable()
            self._use_button.disable()
            self._hold_button.disable()
            self._tm_hm_button.disable()
            self._buy_button.disable()
            self._sell_button.disable()
            return

        cur_item = current_gen_info().item_db().get_item(self._item_selector.get())

        if cur_item is None:
            self._acquire_button.disable()
            self._drop_button.disable()
            self._use_button.disable()
            self._hold_button.disable()
            self._tm_hm_button.disable()
            self._buy_button.disable()
            self._sell_button.disable()
        else:
            if cur_item.move_name is None:
                self._tm_hm_button.disable()
            else:
                self._tm_hm_button.enable()

            if cur_item.name in current_gen_info().get_valid_vitamins() or cur_item.name == const.RARE_CANDY:
                self._use_button.enable()
            else:
                self._use_button.disable()

            if current_gen_info().get_generation() != 1:
                self._hold_button.enable()
            else:
                self._hold_button.disable()

            self._acquire_button.enable()
            self._drop_button.enable()
            self._buy_button.enable()
            self._sell_button.enable()

    def item_filter_callback(self):
        item_type = self._item_type_selector.get()
        backpack_filter = False
        if item_type == const.ITEM_TYPE_BACKPACK_ITEMS:
            item_type = const.ITEM_TYPE_ALL_ITEMS
            backpack_filter = True

        new_vals = current_gen_info().item_db().get_filtered_names(
            item_type=item_type,
            source_mart=self._item_mart_selector.get(),
        )

        if backpack_filter:
            cur_state = self._controller.get_active_state()
            if cur_state is None:
                new_vals = []
            else:
                backpack_items = [x.base_item.name for x in cur_state.inventory.cur_items]
                new_vals = [x for x in new_vals if x in backpack_items]

        item_filter_val = self._item_filter.get().strip().lower()
        if item_filter_val:
            new_vals = [x for x in new_vals if item_filter_val in x.lower()]

        if not new_vals:
            new_vals.append(const.NO_ITEM)

        self._item_selector.new_values(new_vals)

    def item_selector_callback(self):
        if not hasattr(self, '_item_amount'):
            return
        try:
            item_amt = int(self._item_amount.get())
            cur_item = current_gen_info().item_db().get_item(self._item_selector.get())
            self._purchase_cost_amt.setText(f"{cur_item.purchase_price * item_amt}")
            self._sell_cost_amt.setText(f"{cur_item.sell_price * item_amt}")
        except Exception:
            self._purchase_cost_amt.setText("")
            self._sell_cost_amt.setText("")

        self.update_button_status()

    def _acquire_item(self):
        self._create_event(
            EventDefinition(
                item_event_def=InventoryEventDefinition(
                    self._item_selector.get(),
                    int(self._item_amount.get()),
                    True,
                    False,
                )
            )
        )

    def _drop_item(self):
        self._create_event(
            EventDefinition(
                item_event_def=InventoryEventDefinition(
                    self._item_selector.get(),
                    int(self._item_amount.get()),
                    False,
                    False,
                )
            )
        )

    def _buy_item(self):
        self._create_event(
            EventDefinition(
                item_event_def=InventoryEventDefinition(
                    self._item_selector.get(),
                    int(self._item_amount.get()),
                    True,
                    True,
                )
            )
        )

    def _sell_item(self):
        self._create_event(
            EventDefinition(
                item_event_def=InventoryEventDefinition(
                    self._item_selector.get(),
                    int(self._item_amount.get()),
                    False,
                    True,
                )
            )
        )

    def _use_item(self):
        cur_item = self._item_selector.get()
        if cur_item in current_gen_info().get_valid_vitamins():
            self._create_event(
                EventDefinition(
                    vitamin=VitaminEventDefinition(cur_item, int(self._item_amount.get()))
                )
            )
        elif cur_item == const.RARE_CANDY:
            self._create_event(
                EventDefinition(
                    rare_candy=RareCandyEventDefinition(int(self._item_amount.get()))
                )
            )

    def _hold_item(self):
        cur_item = self._item_selector.get()
        self._create_event(
            EventDefinition(
                hold_item=HoldItemEventDefinition(cur_item)
            )
        )

    def _learn_move(self):
        try:
            cur_item = self._item_selector.get()
            move_name = current_gen_info().item_db().get_item(cur_item).move_name
            cur_state = self._controller.get_active_state()

            if cur_item in current_gen_info().item_db().tms:
                self._create_event(
                    EventDefinition(
                        learn_move=LearnMoveEventDefinition(
                            move_name,
                            cur_state.solo_pkmn.get_move_destination(move_name, None)[0],
                            cur_item,
                        )
                    )
                )
        except Exception as e:
            logger.error(f"Silently ignoring error when trying to learn move")
            logger.exception(e)

    def _create_event(self, event_def):
        self._controller.new_event(
            event_def,
            self._controller.get_single_selected_event_id(),
        )
