//! Bounded limits for engineering tools.

pub const MAX_PATH_LEN: usize = 256;
pub const MAX_COMMAND_LEN: usize = 4096;
pub const MAX_OUTPUT_BYTES: usize = 64 * 1024;
pub const MAX_HISTORY: usize = 64;
pub const MAX_TREE_DEPTH: usize = 4;
pub const MAX_FIND_DEPTH: usize = 8;
pub const MAX_FIND_RESULTS: usize = 64;
pub const MAX_GREP_FILES: usize = 32;
pub const MAX_GREP_DEPTH: usize = 4;
pub const MAX_HEAD_TAIL_LINES: usize = 256;
pub const MAX_DEFAULT_LINES: usize = 10;
pub const MAX_EDIT_FILE_BYTES: usize = 32 * 1024;
pub const MAX_EDIT_LINES: usize = 512;
pub const MAX_EDIT_LINE_LEN: usize = 512;
pub const MAX_SCRIPT_BYTES: usize = 16 * 1024;
pub const MAX_SCRIPT_LINES: usize = 256;
pub const MAX_SCRIPT_REPEAT: usize = 1000;
pub const MAX_SCRIPT_DEPTH: usize = 8;
pub const MAX_HEX_DUMP: usize = 4096;
pub const MAX_STRINGS: usize = 256;
pub const MIN_STRING_LEN: usize = 4;
pub const MAX_BUILD_OUTPUT: usize = 16 * 1024;
