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

  it("relatedNotes -> related_notes", () => {
    api.relatedNotes("note-1");
    expect(invokeMock).toHaveBeenCalledWith("related_notes", { id: "note-1" });
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
    expect(invokeMock).toHaveBeenCalledWith("create_note", {
      folderId: "folder-1",
      isTemplate: false,
    });
  });

  it("createNote(folderId, true) -> create_note with isTemplate", () => {
    api.createNote("folder-1", true);
    expect(invokeMock).toHaveBeenCalledWith("create_note", {
      folderId: "folder-1",
      isTemplate: true,
    });
  });

  it("createNoteFromTemplate -> create_note_from_template", () => {
    api.createNoteFromTemplate("folder-1", "template-1");
    expect(invokeMock).toHaveBeenCalledWith("create_note_from_template", {
      folderId: "folder-1",
      templateId: "template-1",
    });
  });

  it("setNoteTemplate -> set_note_template", () => {
    api.setNoteTemplate("note-1", true);
    expect(invokeMock).toHaveBeenCalledWith("set_note_template", {
      id: "note-1",
      isTemplate: true,
    });
  });

  it("getOrCreateDailyNote -> get_or_create_daily_note", () => {
    api.getOrCreateDailyNote("2026-10-05");
    expect(invokeMock).toHaveBeenCalledWith("get_or_create_daily_note", {
      date: "2026-10-05",
    });
  });

  it("moveNote -> move_note", () => {
    api.moveNote("note-1", "folder-2");
    expect(invokeMock).toHaveBeenCalledWith("move_note", { id: "note-1", folderId: "folder-2" });
  });

  it("saveAttachment -> save_attachment with a plain number array", () => {
    api.saveAttachment("note-1", "report.pdf", new Uint8Array([1, 2, 3]));
    expect(invokeMock).toHaveBeenCalledWith("save_attachment", {
      noteId: "note-1",
      filename: "report.pdf",
      bytes: [1, 2, 3],
    });
  });

  it("getAttachmentSize -> get_attachment_size", () => {
    api.getAttachmentSize(".attachments/note-1/report.pdf");
    expect(invokeMock).toHaveBeenCalledWith("get_attachment_size", {
      path: ".attachments/note-1/report.pdf",
    });
  });

  it("openAttachment -> open_attachment", () => {
    api.openAttachment(".attachments/note-1/report.pdf");
    expect(invokeMock).toHaveBeenCalledWith("open_attachment", {
      path: ".attachments/note-1/report.pdf",
    });
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

  it("setFolderColor -> set_folder_color", () => {
    api.setFolderColor("folder-1", "#ff9500");
    expect(invokeMock).toHaveBeenCalledWith("set_folder_color", {
      id: "folder-1",
      color: "#ff9500",
    });
  });

  it("setTagColor -> set_tag_color", () => {
    api.setTagColor("tag-1", "#ff9500");
    expect(invokeMock).toHaveBeenCalledWith("set_tag_color", {
      id: "tag-1",
      color: "#ff9500",
    });
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

  it("listNoteVersions -> list_note_versions", () => {
    api.listNoteVersions("note-1");
    expect(invokeMock).toHaveBeenCalledWith("list_note_versions", { id: "note-1" });
  });

  it("restoreNoteVersion -> restore_note_version", () => {
    api.restoreNoteVersion("note-1", "2026-01-01T00:00:00Z");
    expect(invokeMock).toHaveBeenCalledWith("restore_note_version", {
      id: "note-1",
      timestamp: "2026-01-01T00:00:00Z",
    });
  });

  it("exportFile -> export_file", () => {
    api.exportFile("/tmp/note.html", "<html></html>");
    expect(invokeMock).toHaveBeenCalledWith("export_file", {
      path: "/tmp/note.html",
      content: "<html></html>",
    });
  });

  it("importMarkdownFolder -> import_markdown_folder", () => {
    api.importMarkdownFolder("/home/user/ObsidianVault");
    expect(invokeMock).toHaveBeenCalledWith("import_markdown_folder", {
      path: "/home/user/ObsidianVault",
    });
  });

  it("exportVaultBackup -> export_vault_backup", () => {
    api.exportVaultBackup("/home/user/Notes Backup.zip");
    expect(invokeMock).toHaveBeenCalledWith("export_vault_backup", {
      path: "/home/user/Notes Backup.zip",
    });
  });

  it("restoreVaultBackup -> restore_vault_backup", () => {
    api.restoreVaultBackup("/home/user/Notes Backup.zip", "/home/user/Restored");
    expect(invokeMock).toHaveBeenCalledWith("restore_vault_backup", {
      zipPath: "/home/user/Notes Backup.zip",
      destDir: "/home/user/Restored",
    });
  });

  it("openExternalLink -> open_external_link", () => {
    api.openExternalLink("https://example.com");
    expect(invokeMock).toHaveBeenCalledWith("open_external_link", {
      url: "https://example.com",
    });
  });

  it("vaultStatus -> vault_status", () => {
    api.vaultStatus();
    expect(invokeMock).toHaveBeenCalledWith("vault_status");
  });

  it("enableVaultEncryption -> enable_vault_encryption", () => {
    api.enableVaultEncryption("hunter2");
    expect(invokeMock).toHaveBeenCalledWith("enable_vault_encryption", {
      password: "hunter2",
    });
  });

  it("unlockVaultWithPassword -> unlock_vault_with_password", () => {
    api.unlockVaultWithPassword("hunter2");
    expect(invokeMock).toHaveBeenCalledWith("unlock_vault_with_password", {
      password: "hunter2",
    });
  });

  it("unlockVaultWithRecoveryKey -> unlock_vault_with_recovery_key", () => {
    api.unlockVaultWithRecoveryKey("AB12-CD34");
    expect(invokeMock).toHaveBeenCalledWith("unlock_vault_with_recovery_key", {
      recoveryKey: "AB12-CD34",
    });
  });

  it("lockVault -> lock_vault", () => {
    api.lockVault();
    expect(invokeMock).toHaveBeenCalledWith("lock_vault");
  });
});
