//! Pure domain for the Gloss shell. Commands and projections only.
//! File bytes move through gloss-cli via Process so reduce stays pure.

mod commands;
mod note;
mod paths;
mod projections;
mod types;

pub use commands::{
    apply_create_cmd, apply_rename_cmd, apply_replace_cmd, backlinks_cmd, cat_cmd, list_cmd,
    parse_list, rules_cmd, undo_cmd,
};
pub use note::{
    apply_loaded_note, assemble_note, file_stem, frontmatter_scan_rows, frontmatter_title,
    full_note_text, note_snippet, note_title, prepend_title_heading, split_note,
    strip_leading_title_heading,
};
pub use paths::{
    daily_note_content, default_daily_note_path, expand_ancestors_into, is_folder_id, journal_path,
    renamed_note_path, resolve_daily_note, resolve_wikilink, title_needs_rename, today_local_iso, unique_folder_note,
    unique_inbox_path, vault_name,
};
pub use projections::{
    char_count_label, insert_wikilink_mention, mention_trigger, project_bookmark_rows,
    project_file_rows, project_link_rows, project_palette_rows, project_preview,
    project_search_rows, rules_preview, save_busy, save_ok, word_count_label,
};
pub use types::{
    parse_workspace_bytes, upsert_vault, FileRow, FrontmatterField, IoKind, NoteRow, Pending,
    SidebarView, TabRow, VaultRecord, VaultRow, WorkspaceSettings, AUTOSAVE_MS, STORE_KEY,
    TOAST_MS,
};
