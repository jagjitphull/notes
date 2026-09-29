import { beforeEach, describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

const api = await import("./api");

// Each Tauri command name and its argument shape is duplicated by hand on
// the Rust side (#[tauri::command] fn name + params) and here - nothing
// keeps the two in sync at compile time. These tests exist to catch a
// silent mismatch (wrong command name, wrong arg key) that would
// otherwise only surface as a runtime "command not found" in the app.
describe("api.ts Tauri command bindings", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("searchNotes -> search_notes", () => {
    api.searchNotes("hello");
    expect(invokeMock).toHaveBeenCalledWith("search_notes", { query: "hello" });
  });

  it("smartSearch -> smart_search", () => {
    api.smartSearch("hello");
    expect(invokeMock).toHaveBeenCalledWith("smart_search", { query: "hello" });
  });

  it("smartSearchAvailable -> smart_search_available with no args", () => {
    api.smartSearchAvailable();
    expect(invokeMock).toHaveBeenCalledWith("smart_search_available");
  });

  it("addNoteTag -> add_note_tag", () => {
    api.addNoteTag("note-1", "urgent");
    expect(invokeMock).toHaveBeenCalledWith("add_note_tag", { id: "note-1", tagName: "urgent" });
  });

  it("removeNoteTag -> remove_note_tag", () => {
    api.removeNoteTag("note-1", "urgent");
    expect(invokeMock).toHaveBeenCalledWith("remove_note_tag", {
      id: "note-1",
      tagName: "urgent",
    });
  });

  it("setNotePinned -> set_note_pinned", () => {
    api.setNotePinned("note-1", true);
    expect(invokeMock).toHaveBeenCalledWith("set_note_pinned", { id: "note-1", pinned: true });
  });

  it("setNoteDeleted -> set_note_deleted", () => {
    api.setNoteDeleted("note-1", true);
    expect(invokeMock).toHaveBeenCalledWith("set_note_deleted", { id: "note-1", deleted: true });
  });

  it("createNote -> create_note", () => {
    api.createNote("folder-1");
    expect(invokeMock).toHaveBeenCalledWith("create_note", { folderId: "folder-1" });
  });

  it("moveNote -> move_note", () => {
    api.moveNote("note-1", "folder-2");
    expect(invokeMock).toHaveBeenCalledWith("move_note", { id: "note-1", folderId: "folder-2" });
  });

  it("createFolder -> create_folder", () => {
    api.createFolder("parent-1", "Work");
    expect(invokeMock).toHaveBeenCalledWith("create_folder", {
      parentId: "parent-1",
      name: "Work",
    });
  });

  it("renameFolder -> rename_folder", () => {
    api.renameFolder("folder-1", "Projects");
    expect(invokeMock).toHaveBeenCalledWith("rename_folder", { id: "folder-1", name: "Projects" });
  });

  it("deleteFolder -> delete_folder", () => {
    api.deleteFolder("folder-1");
    expect(invokeMock).toHaveBeenCalledWith("delete_folder", { id: "folder-1" });
  });

  it("getNoteBody -> get_note_body", () => {
    api.getNoteBody("note-1");
    expect(invokeMock).toHaveBeenCalledWith("get_note_body", { id: "note-1" });
  });

  it("saveNoteBody -> save_note_body", () => {
    api.saveNoteBody("note-1", "body text");
    expect(invokeMock).toHaveBeenCalledWith("save_note_body", {
      id: "note-1",
      body: "body text",
    });
  });

  it("deleteNotePermanently -> delete_note_permanently", () => {
    api.deleteNotePermanently("note-1");
    expect(invokeMock).toHaveBeenCalledWith("delete_note_permanently", { id: "note-1" });
  });

  it("setNotesRoot -> set_notes_root", () => {
    api.setNotesRoot("/home/user/notes");
    expect(invokeMock).toHaveBeenCalledWith("set_notes_root", { path: "/home/user/notes" });
  });
});
