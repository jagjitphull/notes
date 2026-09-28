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

export function getNoteBody(id: string): Promise<string> {
  return invoke("get_note_body", { id });
}

export function saveNoteBody(id: string, body: string): Promise<void> {
  return invoke("save_note_body", { id, body });
}

export function createNote(folderId: string): Promise<string> {
  return invoke("create_note", { folderId });
}

export function setNotePinned(id: string, pinned: boolean): Promise<void> {
  return invoke("set_note_pinned", { id, pinned });
}

export function setNoteDeleted(id: string, deleted: boolean): Promise<void> {
  return invoke("set_note_deleted", { id, deleted });
}
