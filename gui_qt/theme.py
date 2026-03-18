import os

from utils.config_manager import config

# Compute asset paths once (Qt needs forward slashes even on Windows).
_ASSETS_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "assets",
)
_DARK_THEME_DIR = os.path.join(_ASSETS_DIR, "theme", "dark")
# Checkbox images from the dark tkinter theme — reused for Qt indicators.
_CHECK_BASIC = os.path.join(_DARK_THEME_DIR, "check-basic.gif").replace("\\", "/")
_CHECK_HOVER = os.path.join(_DARK_THEME_DIR, "check-hover.gif").replace("\\", "/")
_CHECK_ACCENT = os.path.join(_DARK_THEME_DIR, "check-accent.gif").replace("\\", "/")
_BOX_BASIC = os.path.join(_DARK_THEME_DIR, "box-basic.gif").replace("\\", "/")
_BOX_HOVER = os.path.join(_DARK_THEME_DIR, "box-hover.gif").replace("\\", "/")
_BOX_ACCENT = os.path.join(_DARK_THEME_DIR, "box-accent.gif").replace("\\", "/")

# Branch arrow images for tree expand/collapse.
_ARROW_RIGHT = os.path.join(_DARK_THEME_DIR, "right.gif").replace("\\", "/")
_ARROW_DOWN = os.path.join(_DARK_THEME_DIR, "down.gif").replace("\\", "/")


def generate_stylesheet():
    bg = config.get_background_color()
    text = config.get_text_color()
    success = config.get_success_color()
    warning = config.get_warning_color()
    failure = config.get_failure_color()
    divider = config.get_divider_color()
    header = config.get_header_color()
    primary = config.get_primary_color()
    secondary = config.get_secondary_color()
    contrast = config.get_contrast_color()
    font_name = config.get_custom_font_name()

    # Derive colors from base -- higher contrast for true dark mode
    bg_lighter = _lighten(bg, 0.06)
    bg_darker = _darken(bg, 0.10)
    bg_input = _lighten(bg, 0.04)
    border_color = _lighten(bg, 0.22)
    border_focus = _lighten(bg, 0.35)
    accent = "#0078d4"
    accent_hover = "#1a8ae8"
    accent_pressed = "#005fa3"
    hover_bg = _lighten(bg, 0.10)
    disabled_text = _lighten(bg, 0.30)
    subtle_border = _lighten(bg, 0.12)

    return f"""
/* ===== Global ===== */
QWidget {{
    background-color: {bg};
    color: {text};
    font-family: "{font_name}";
    font-size: 9pt;
    outline: none;
}}
QMainWindow {{
    background-color: {bg};
}}

/* ===== Labels ===== */
QLabel {{
    background-color: transparent;
    padding: 0px;
    border: none;
}}
QLabel[class="success"] {{ color: {success}; }}
QLabel[class="warning"] {{ color: {warning}; }}
QLabel[class="failure"] {{ color: {failure}; }}
QLabel[class="header"] {{ color: {header}; font-weight: bold; }}
QLabel[class="primary"] {{ color: {primary}; }}
QLabel[class="secondary"] {{ color: {secondary}; }}
QLabel[class="contrast"] {{ color: {contrast}; }}
QLabel[class="divider"] {{ color: {divider}; }}
QLabel[class="title"] {{
    font-size: 22pt;
    font-weight: bold;
}}

/* ===== Buttons ===== */
QPushButton {{
    background-color: {bg_lighter};
    color: {text};
    border: 1px solid {border_color};
    border-radius: 3px;
    padding: 2px 8px;
    min-height: 18px;
}}
QPushButton:hover {{
    background-color: {hover_bg};
    border-color: {accent};
}}
QPushButton:pressed {{
    background-color: {accent_pressed};
    border-color: {accent};
    color: #ffffff;
}}
QPushButton:disabled {{
    color: {disabled_text};
    background-color: {bg_darker};
    border-color: {subtle_border};
}}
QPushButton:checked {{
    background-color: {accent};
    border-color: {accent};
    color: #ffffff;
}}
QPushButton:checked:hover {{
    background-color: {accent_hover};
    border-color: {accent_hover};
}}
QPushButton[class="large"] {{
    padding: 6px 16px;
    font-size: 11pt;
}}

/* Amount entry +/- buttons */
QPushButton[class="amount-btn"] {{
    background-color: {bg_lighter};
    border: 1px solid {border_color};
    border-radius: 2px;
    padding: 0px;
    font-weight: bold;
    font-size: 10pt;
}}
QPushButton[class="amount-btn"]:hover {{
    background-color: {accent};
    border-color: {accent};
    color: #ffffff;
}}
QPushButton[class="amount-btn"]:pressed {{
    background-color: {accent_pressed};
}}
QPushButton[class="amount-btn"]:disabled {{
    color: {disabled_text};
    background-color: {bg_darker};
    border-color: {subtle_border};
}}

/* ===== Segmented Toggle Buttons ===== */
QPushButton[class="seg-toggle"] {{
    background-color: {bg_darker};
    color: {secondary};
    border: 1px solid {border_color};
    border-radius: 0px;
    padding: 4px 14px;
    font-weight: bold;
    min-height: 22px;
}}
QPushButton[class="seg-toggle"]:hover {{
    background-color: {hover_bg};
    color: {text};
    border-color: {border_color};
}}
QPushButton[class="seg-toggle"]:checked {{
    background-color: {bg_lighter};
    color: #ffffff;
    border-color: {border_color};
    border-left: 3px solid {failure};
}}
QPushButton[class="seg-toggle"]:checked:hover {{
    background-color: {hover_bg};
    border-left: 3px solid {failure};
}}

/* ===== Line Edits ===== */
QLineEdit {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    border-radius: 2px;
    padding: 2px 4px;
    selection-background-color: {accent};
    selection-color: #ffffff;
}}
QLineEdit:focus {{
    border-color: {accent};
}}
QLineEdit:disabled {{
    color: {disabled_text};
    background-color: {bg_darker};
    border-color: {subtle_border};
}}

/* ===== Text Edits ===== */
QPlainTextEdit {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    border-radius: 2px;
    padding: 2px;
    selection-background-color: {accent};
    selection-color: #ffffff;
}}
QPlainTextEdit:focus {{
    border-color: {accent};
}}

/* ===== Combo Boxes ===== */
QComboBox {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    border-radius: 2px;
    padding: 2px 22px 2px 6px;
    min-height: 18px;
}}
QComboBox:hover {{
    border-color: {accent};
}}
QComboBox:focus {{
    border-color: {accent};
}}
QComboBox::drop-down {{
    subcontrol-origin: padding;
    subcontrol-position: top right;
    width: 20px;
    border-left: 1px solid {border_color};
}}
QComboBox::down-arrow {{
    image: url({_ARROW_DOWN});
}}
QComboBox QAbstractItemView {{
    background-color: {bg_lighter};
    color: {text};
    selection-background-color: {accent};
    selection-color: #ffffff;
    border: 1px solid {border_color};
    outline: none;
    padding: 2px;
}}
QComboBox QAbstractItemView::item {{
    min-height: 20px;
    padding: 2px 4px;
}}
QComboBox QAbstractItemView::item:hover {{
    background-color: {hover_bg};
}}
QComboBox:disabled {{
    color: {disabled_text};
    background-color: {bg_darker};
    border-color: {subtle_border};
}}

/* ===== Spin Boxes ===== */
QSpinBox {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    border-radius: 2px;
    padding: 1px 3px;
}}
QSpinBox:focus {{
    border-color: {accent};
}}

/* ===== Check Boxes ===== */
QCheckBox {{
    spacing: 4px;
    background-color: transparent;
}}
QCheckBox::indicator {{
    width: 16px;
    height: 16px;
    border: none;
}}
QCheckBox::indicator:checked {{
    image: url({_CHECK_BASIC});
}}
QCheckBox::indicator:checked:hover {{
    image: url({_CHECK_HOVER});
}}
QCheckBox::indicator:checked:focus {{
    image: url({_CHECK_ACCENT});
}}
QCheckBox::indicator:unchecked {{
    image: url({_BOX_BASIC});
}}
QCheckBox::indicator:unchecked:hover {{
    image: url({_BOX_HOVER});
}}
QCheckBox::indicator:unchecked:focus {{
    image: url({_BOX_ACCENT});
}}

/* ===== Radio Buttons ===== */
QRadioButton {{
    spacing: 4px;
    background-color: transparent;
}}
QRadioButton::indicator {{
    width: 14px;
    height: 14px;
    border: 1px solid {border_focus};
    border-radius: 8px;
    background-color: {bg_input};
}}
QRadioButton::indicator:checked {{
    background-color: {accent};
    border-color: {accent};
}}
QRadioButton::indicator:hover {{
    border-color: {accent};
}}

/* ===== Group Boxes ===== */
QGroupBox {{
    border: 1px solid {border_color};
    border-radius: 3px;
    margin-top: 6px;
    padding-top: 6px;
    font-weight: bold;
}}
QGroupBox::title {{
    subcontrol-origin: margin;
    subcontrol-position: top left;
    padding: 0px 4px;
    color: {primary};
}}

/* ===== Tab Widget ===== */
QTabWidget::pane {{
    border: 1px solid {border_color};
    background-color: {bg};
    top: -1px;
}}
QTabBar::tab {{
    background-color: {bg_darker};
    color: {text};
    border: 1px solid {border_color};
    border-bottom: none;
    border-top-left-radius: 3px;
    border-top-right-radius: 3px;
    padding: 3px 12px;
    margin-right: 1px;
}}
QTabBar::tab:selected {{
    background-color: {bg};
    border-bottom: 2px solid {accent};
    color: #ffffff;
}}
QTabBar::tab:hover:!selected {{
    background-color: {hover_bg};
}}

/* ===== Tree View ===== */
QTreeView {{
    background-color: {bg_lighter};
    color: {text};
    border: 1px solid {border_color};
    selection-background-color: {accent};
    outline: none;
}}
QTreeView::item {{
    padding: 0px 2px;
    min-height: 18px;
}}
QTreeView::item:selected {{
    background-color: {accent};
    color: #ffffff;
}}
QTreeView::item:hover:!selected {{
    background-color: {hover_bg};
}}
QTreeView::branch:selected {{
    background-color: {accent};
}}
QTreeView::branch:hover:!selected {{
    background-color: {hover_bg};
}}
QTreeView::branch:has-children:!has-siblings:closed,
QTreeView::branch:closed:has-children:has-siblings {{
    image: url({_ARROW_RIGHT});
}}
QTreeView::branch:open:has-children:!has-siblings,
QTreeView::branch:open:has-children:has-siblings {{
    image: url({_ARROW_DOWN});
}}
QTreeView::indicator {{
    width: 16px;
    height: 16px;
    border: none;
}}
QTreeView::indicator:checked {{
    image: url({_CHECK_BASIC});
}}
QTreeView::indicator:checked:hover {{
    image: url({_CHECK_HOVER});
}}
QTreeView::indicator:unchecked {{
    image: url({_BOX_BASIC});
}}
QTreeView::indicator:unchecked:hover {{
    image: url({_BOX_HOVER});
}}
QHeaderView::section {{
    background-color: {bg_darker};
    color: {text};
    border: 1px solid {border_color};
    padding: 2px 4px;
    font-weight: bold;
}}

/* ===== Scroll Bars ===== */
QScrollBar:vertical {{
    background-color: {bg};
    width: 10px;
    margin: 0;
    border: none;
}}
QScrollBar::handle:vertical {{
    background-color: {border_color};
    border-radius: 4px;
    min-height: 24px;
    margin: 1px;
}}
QScrollBar::handle:vertical:hover {{
    background-color: {divider};
}}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{
    height: 0;
    border: none;
    background: none;
}}
QScrollBar::add-page:vertical, QScrollBar::sub-page:vertical {{
    background: none;
}}
QScrollBar:horizontal {{
    background-color: {bg};
    height: 10px;
    margin: 0;
    border: none;
}}
QScrollBar::handle:horizontal {{
    background-color: {border_color};
    border-radius: 4px;
    min-width: 24px;
    margin: 1px;
}}
QScrollBar::handle:horizontal:hover {{
    background-color: {divider};
}}
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {{
    width: 0;
    border: none;
    background: none;
}}
QScrollBar::add-page:horizontal, QScrollBar::sub-page:horizontal {{
    background: none;
}}
QScrollArea {{
    border: none;
}}

/* ===== Menu Bar ===== */
QMenuBar {{
    background-color: {bg_darker};
    color: {text};
    border-bottom: 1px solid {border_color};
    padding: 1px 0px;
}}
QMenuBar::item {{
    padding: 2px 8px;
    background-color: transparent;
}}
QMenuBar::item:selected {{
    background-color: {hover_bg};
}}
QMenu {{
    background-color: {bg_lighter};
    color: {text};
    border: 1px solid {border_color};
    padding: 2px 0px;
}}
QMenu::item {{
    padding: 3px 24px 3px 8px;
}}
QMenu::item:selected {{
    background-color: {accent};
    color: #ffffff;
}}
QMenu::item:disabled {{
    color: {disabled_text};
}}
QMenu::separator {{
    height: 1px;
    background-color: {border_color};
    margin: 2px 6px;
}}
QMenu::indicator {{
    width: 16px;
    height: 16px;
    margin-left: 4px;
    border: none;
}}
QMenu::indicator:checked {{
    image: url({_CHECK_BASIC});
}}
QMenu::indicator:unchecked {{
    image: url({_BOX_BASIC});
}}

/* ===== Dialogs ===== */
QDialog {{
    background-color: {bg};
}}

/* ===== Splitter ===== */
QSplitter::handle {{
    background-color: {border_color};
}}
QSplitter::handle:horizontal {{
    width: 3px;
}}
QSplitter::handle:vertical {{
    height: 3px;
}}
QSplitter::handle:hover {{
    background-color: {accent};
}}

/* ===== Status Bar ===== */
QStatusBar {{
    background-color: {bg_darker};
    color: {secondary};
    border-top: 1px solid {border_color};
}}
QStatusBar::item {{
    border: none;
}}

/* ===== Progress Bar ===== */
QProgressBar {{
    background-color: {bg_input};
    border: 1px solid {border_color};
    border-radius: 3px;
    text-align: center;
    color: {text};
}}
QProgressBar::chunk {{
    background-color: {accent};
    border-radius: 2px;
}}

/* ===== Tool Tip ===== */
QToolTip {{
    background-color: {bg_lighter};
    color: {text};
    border: 1px solid {border_color};
    padding: 3px 6px;
}}

/* ===== QTreeWidget (game selection tables) ===== */
QTreeWidget {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    selection-background-color: {accent};
    selection-color: #ffffff;
    outline: none;
}}
QTreeWidget::item {{
    padding: 2px 4px;
}}
QTreeWidget::item:selected {{
    background-color: {accent};
    color: #ffffff;
}}
QTreeWidget::item:hover:!selected {{
    background-color: {hover_bg};
}}

/* ===== QFrame separators ===== */
QFrame[frameShape="4"] {{
    color: {border_color};
    max-height: 1px;
}}
QFrame[frameShape="5"] {{
    color: {border_color};
    max-width: 1px;
}}
"""


def _hex_to_rgb(hex_color):
    hex_color = hex_color.lstrip('#')
    if len(hex_color) == 6:
        return tuple(int(hex_color[i:i+2], 16) for i in (0, 2, 4))
    return (128, 128, 128)


def _rgb_to_hex(r, g, b):
    return f"#{int(r):02x}{int(g):02x}{int(b):02x}"


def _lighten(hex_color, amount):
    r, g, b = _hex_to_rgb(hex_color)
    r = min(255, r + (255 - r) * amount)
    g = min(255, g + (255 - g) * amount)
    b = min(255, b + (255 - b) * amount)
    return _rgb_to_hex(r, g, b)


def _darken(hex_color, amount):
    r, g, b = _hex_to_rgb(hex_color)
    r = max(0, r * (1 - amount))
    g = max(0, g * (1 - amount))
    b = max(0, b * (1 - amount))
    return _rgb_to_hex(r, g, b)
