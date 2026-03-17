from utils.config_manager import config


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

    # Derive colors from base
    bg_lighter = _lighten(bg, 0.08)
    bg_darker = _darken(bg, 0.05)
    bg_input = _lighten(bg, 0.12)
    border_color = _lighten(bg, 0.2)
    accent = "#0078d4"
    hover_bg = _lighten(bg, 0.15)
    disabled_text = _lighten(bg, 0.35)

    return f"""
/* ===== Global ===== */
QWidget {{
    background-color: {bg};
    color: {text};
    font-family: "{font_name}";
    font-size: 9pt;
}}
QMainWindow {{
    background-color: {bg};
}}

/* ===== Labels ===== */
QLabel {{
    background-color: transparent;
    padding: 0px;
}}
QLabel[class="success"] {{ color: {success}; }}
QLabel[class="warning"] {{ color: {warning}; }}
QLabel[class="failure"] {{ color: {failure}; }}
QLabel[class="header"] {{ color: {header}; }}
QLabel[class="primary"] {{ color: {primary}; }}
QLabel[class="secondary"] {{ color: {secondary}; }}
QLabel[class="contrast"] {{ color: {contrast}; }}
QLabel[class="divider"] {{ color: {divider}; }}
QLabel[class="title"] {{
    font-size: 24pt;
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
    background-color: {accent};
}}
QPushButton:disabled {{
    color: {disabled_text};
    background-color: {bg_darker};
    border-color: {bg_darker};
}}
QPushButton[class="large"] {{
    padding: 8px 16px;
    font-size: 11pt;
}}

/* ===== Line Edits ===== */
QLineEdit {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    border-radius: 2px;
    padding: 2px 4px;
    selection-background-color: {accent};
}}
QLineEdit:focus {{
    border-color: {accent};
}}
QLineEdit:disabled {{
    color: {disabled_text};
    background-color: {bg_darker};
}}

/* ===== Text Edits ===== */
QPlainTextEdit {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    border-radius: 2px;
    padding: 2px;
    selection-background-color: {accent};
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
    padding: 2px 20px 2px 4px;
    min-height: 18px;
}}
QComboBox:hover {{
    border-color: {accent};
}}
QComboBox::drop-down {{
    subcontrol-origin: padding;
    subcontrol-position: top right;
    width: 18px;
    border-left: 1px solid {border_color};
    border-top-right-radius: 2px;
    border-bottom-right-radius: 2px;
    background-color: {bg_lighter};
}}
QComboBox::down-arrow {{
    width: 0;
    height: 0;
    border-left: 4px solid transparent;
    border-right: 4px solid transparent;
    border-top: 5px solid {text};
}}
QComboBox QAbstractItemView {{
    background-color: {bg_lighter};
    color: {text};
    selection-background-color: {accent};
    selection-color: white;
    border: 1px solid {border_color};
    outline: none;
}}
QComboBox:disabled {{
    color: {disabled_text};
    background-color: {bg_darker};
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
    width: 14px;
    height: 14px;
    border: 1px solid {border_color};
    border-radius: 3px;
    background-color: {bg_input};
}}
QCheckBox::indicator:checked {{
    background-color: {accent};
    border-color: {accent};
}}
QCheckBox::indicator:hover {{
    border-color: {accent};
}}

/* ===== Radio Buttons ===== */
QRadioButton {{
    spacing: 4px;
    background-color: transparent;
}}
QRadioButton::indicator {{
    width: 14px;
    height: 14px;
    border: 1px solid {border_color};
    border-radius: 8px;
    background-color: {bg_input};
}}
QRadioButton::indicator:checked {{
    background-color: {accent};
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
}}
QTabBar::tab:hover:!selected {{
    background-color: {hover_bg};
}}

/* ===== Tree View ===== */
QTreeView {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    alternate-background-color: {bg_lighter};
    selection-background-color: {accent};
    outline: none;
}}
QTreeView::item {{
    padding: 0px 2px;
    min-height: 18px;
}}
QTreeView::item:selected {{
    background-color: {accent};
    color: white;
}}
QTreeView::item:hover:!selected {{
    background-color: {hover_bg};
}}
QTreeView::branch {{
    background-color: transparent;
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
}}
QScrollBar:horizontal {{
    background-color: {bg};
    height: 10px;
    margin: 0;
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
}}
QMenu::item {{
    padding: 3px 24px 3px 8px;
}}
QMenu::item:selected {{
    background-color: {accent};
    color: white;
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
    width: 14px;
    height: 14px;
    margin-left: 4px;
    border: 1px solid {border_color};
    border-radius: 3px;
    background-color: {bg_input};
}}
QMenu::indicator:checked {{
    background-color: {accent};
    border-color: {accent};
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

/* ===== Status Bar ===== */
QStatusBar {{
    background-color: {bg_darker};
    color: {secondary};
    border-top: 1px solid {border_color};
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
    padding: 3px;
}}

/* ===== QTreeWidget (game selection tables) ===== */
QTreeWidget {{
    background-color: {bg_input};
    color: {text};
    border: 1px solid {border_color};
    selection-background-color: {accent};
    selection-color: white;
    outline: none;
}}
QTreeWidget::item {{
    padding: 2px 4px;
}}
QTreeWidget::item:selected {{
    background-color: {accent};
    color: white;
}}
QTreeWidget::item:hover:!selected {{
    background-color: {hover_bg};
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
