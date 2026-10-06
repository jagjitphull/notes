import { invoke } from "@tauri-apps/api/core";
import type { Folder, Note, Tag } from "./types";

export function getNotesRoot(): Promise<string | null> {
  return invoke("get_notes_root");
}

export interface CloudFolderSuggestion {
  name: string;
  path: string;
}

export function detectCloudFolders(): Promise<CloudFolderSuggestion[]> {
  return invoke("detect_cloud_folders");
}

export function setNotesRoot(path: string): Promise<void> {
  return invoke("set_notes_root", { path });
}

export function listFolders(): Promise<Folder[]> {
  return invoke("list_folders");
}

export function listNotes(): Promise<Note[]> {
  return invoke("list_notes");
}

export function listTags(): Promise<Tag[]> {
  return invoke("list_tags");
}

export function searchNotes(query: string): Promise<Note[]> {
  return invoke("search_notes", { query });
}

export function smartSearchAvailable(): Promise<boolean> {
  return invoke("smart_search_available");
}

export function smartSearch(query: string): Promise<Note[]> {
  return invoke("smart_search", { query });
}

export function relatedNotes(id: string): Promise<Note[]> {
  return invoke("related_notes", { id });
}

export function getNoteBody(id: string): Promise<string> {
  return invoke("get_note_body", { id });
}

// A conflict means the file changed externally (e.g. synced in from
// another device) since this editor last loaded it, so the save was
// redirected into a new note instead of overwriting that change - see
// store::save_note_body on the Rust side.
export type SaveNoteBodyResult =
  | { outcome: "saved" }
  | { outcome: "conflict"; conflictedNoteId: string; conflictedTitle: string; originalBody: string };

export function saveNoteBody(id: string, body: string): Promise<SaveNoteBodyResult> {
  return invoke("save_note_body", { id, body });
}

export interface NoteVersionInfo {
  timestamp: string;
  preview: string;
}

export function listNoteVersions(id: string): Promise<NoteVersionInfo[]> {
  return invoke("list_note_versions", { id });
}

// Returns the note's body after the restore, to load straight into the
// editor without a second round trip.
export function restoreNoteVersion(id: string, timestamp: string): Promise<string> {
  return invoke("restore_note_version", { id, timestamp });
}

export function exportFile(path: string, content: string): Promise<void> {
  return invoke("export_file", { path, content });
}

// Returns the number of .md/.markdown files successfully imported.
export function importMarkdownFolder(path: string): Promise<number> {
  return invoke("import_markdown_folder", { path });
}

export function createNote(folderId: string, isTemplate = false): Promise<string> {
  return invoke("create_note", { folderId, isTemplate });
}

export function createNoteFromTemplate(folderId: string, templateId: string): Promise<string> {
  return invoke("create_note_from_template", { folderId, templateId });
}

export function setNoteTemplate(id: string, isTemplate: boolean): Promise<void> {
  return invoke("set_note_template", { id, isTemplate });
}

export function getOrCreateDailyNote(date: string): Promise<string> {
  return invoke("get_or_create_daily_note", { date });
}

export function setNotePinned(id: string, pinned: boolean): Promise<void> {
  return invoke("set_note_pinned", { id, pinned });
}

export function addNoteTag(id: string, tagName: string): Promise<void> {
  return invoke("add_note_tag", { id, tagName });
}

export function removeNoteTag(id: string, tagName: string): Promise<void> {
  return invoke("remove_note_tag", { id, tagName });
}

export function setNoteDeleted(id: string, deleted: boolean): Promise<void> {
  return invoke("set_note_deleted", { id, deleted });
}

export function deleteNotePermanently(id: string): Promise<void> {
  return invoke("delete_note_permanently", { id });
}

export function moveNote(id: string, folderId: string): Promise<void> {
  return invoke("move_note", { id, folderId });
}

export interface AttachmentInfo {
  path: string;
  name: string;
  size: number;
}

export function saveAttachment(
  noteId: string,
  filename: string,
  bytes: Uint8Array,
): Promise<AttachmentInfo> {
  // Array.from converts to a plain number array - a Uint8Array would
  // otherwise JSON.stringify as {"0":1,"1":2,...}, not a JSON array, and
  // fail to deserialize into the Rust side's Vec<u8>.
  return invoke("save_attachment", { noteId, filename, bytes: Array.from(bytes) });
}

export function getAttachmentSize(path: string): Promise<number> {
  return invoke("get_attachment_size", { path });
}

export function openAttachment(path: string): Promise<void> {
  return invoke("open_attachment", { path });
}

export function createFolder(parentId: string, name: string): Promise<string> {
  return invoke("create_folder", { parentId, name });
}

export function renameFolder(id: string, name: string): Promise<string> {
  return invoke("rename_folder", { id, name });
}

export function deleteFolder(id: string): Promise<void> {
  return invoke("delete_folder", { id });
}

export function setFolderColor(id: string, color: string | null): Promise<void> {
  return invoke("set_folder_color", { id, color });
}

export function setTagColor(id: string, color: string | null): Promise<void> {
  return invoke("set_tag_color", { id, color });
}
