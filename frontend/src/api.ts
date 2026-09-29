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

export function createFolder(parentId: string, name: string): Promise<string> {
  return invoke("create_folder", { parentId, name });
}

export function renameFolder(id: string, name: string): Promise<string> {
  return invoke("rename_folder", { id, name });
}

export function deleteFolder(id: string): Promise<void> {
  return invoke("delete_folder", { id });
}
