//! Colours of the editor chrome. Text tokens are checked against the surfaces
//! they sit on to reach WCAG AA (4.5:1); the score ink lives in `paint`.

// Surfaces, darkest first.
pub const BG: u32 = 0x131417;
pub const PANEL: u32 = 0x18191d;
pub const POPOVER: u32 = 0x1d1e23;
pub const CARD: u32 = 0x1f2026;
pub const FIELD: u32 = 0x222329;
pub const CANVAS: u32 = 0x24252a;
pub const TRACK: u32 = 0x1b1d22;
pub const HOVER: u32 = 0x2c2e36;
pub const FIELD_HOVER: u32 = 0x30343e;
pub const SCRIM: u32 = 0x08090ccc;

// Lines.
pub const BORDER: u32 = 0x2b2d35;
pub const OUTLINE: u32 = 0x33353e;
pub const MINOR_TICK: u32 = 0x393d47;
pub const POPOVER_BORDER: u32 = 0x3a3d46;
pub const STRONG_BORDER: u32 = 0x424652;
pub const PENDING_OUTLINE: u32 = 0x4a4d58;
pub const TICK: u32 = 0x606572;

// Text.
pub const TEXT: u32 = 0xe8e9ee;
pub const SECONDARY: u32 = 0xbfc3ce;
pub const MUTED: u32 = 0x969ba7;
/// Placeholders in text fields: 5.6:1 on `FIELD`.
pub const PLACEHOLDER: u32 = MUTED;

// The one accent: selection, the active state and focus.
pub const BLUE: u32 = 0x79a4ff;
pub const ACCENT_TEXT: u32 = 0xb8ceff;
pub const SELECT: u32 = 0x22314d;
pub const SELECT_BORDER: u32 = 0x3c5d94;
pub const HEADER_HOVER: u32 = 0x79a4ff14;
pub const TEXT_SELECTION: u32 = 0x79a4ff40;

// Status text, all above 7:1 on the panels.
pub const WARNING: u32 = 0xf2b282;
pub const ERROR: u32 = 0xf1a4a4;
pub const ERROR_BG: u32 = 0x362126;
pub const SUCCESS: u32 = 0x85c6a2;

// Title block on the paper (`0xfbf9f3`).
pub const PAPER_INK: u32 = 0x242323;
pub const PAPER_SECONDARY: u32 = 0x4f4b45;
pub const PAPER_FINE: u32 = 0x67635c;
/// Empty credits hint: 5.1:1, lighter than `PAPER_FINE` but still readable.
pub const PAPER_PLACEHOLDER: u32 = 0x6f6a62;
