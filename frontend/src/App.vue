<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import Icon from "./components/icons/Icon.vue";
import Sidebar from "./components/Sidebar.vue";
import NoteList from "./components/NoteList.vue";
import Editor from "./components/Editor.vue";
import FirstRunSetup from "./components/FirstRunSetup.vue";
import ContextMenu, { type ContextMenuItem } from "./components/ContextMenu.vue";
import PromptModal from "./components/PromptModal.vue";
import CommandPalette, { type PaletteAction } from "./components/CommandPalette.vue";
import GraphView from "./components/GraphView.vue";
import {
  addNoteTag,
  createFolder,
  createNote,
  createNoteFromTemplate,
  deleteFolder,
  deleteNotePermanently,
  getNoteBody,
  getNotesRoot,
  exportVaultBackup,
  getOrCreateDailyNote,
  importMarkdownFolder,
  listFolders,
  restoreVaultBackup,
  setNotesRoot,
  listNotes,
  listTags,
  moveNote,
  removeNoteTag,
  renameFolder,
  saveNoteBody,
  searchNotes,
  setFolderColor,
  setNoteDeleted,
  setNotePinned,
  setNoteTemplate,
  setTagColor,
  smartSearch,
  smartSearchAvailable,
} from "./api";
import { useAppUpdater } from "./composables/useAppUpdater";
import { usePaneLayout } from "./composables/usePaneLayout";
import { useSortPreference } from "./composables/useSortPreference";
import { useTheme } from "./composables/useTheme";
import { useWindowControls } from "./composables/useWindowControls";
import type { Folder, Note, Tag } from "./types";

// A fixed accent palette (matching the system-color style Apple Notes/
// Finder use for tags and folders) rather than a free-form color picker -
// keeps every folder/tag's tint one of a small, visually distinct set.
// Shared between the folder and tag color pickers below.
const COLOR_PALETTE = [
  { value: "#ff3b30", labelKey: "contextMenu.colorRed" },
  { value: "#ff9500", labelKey: "contextMenu.colorOrange" },
  { value: "#ffcc00", labelKey: "contextMenu.colorYellow" },
  { value: "#34c759", labelKey: "contextMenu.colorGreen" },
  { value: "#0a84ff", labelKey: "contextMenu.colorBlue" },
  { value: "#5e5ce6", labelKey: "contextMenu.colorIndigo" },
  { value: "#af52de", labelKey: "contextMenu.colorPurple" },
  { value: "#8e8e93", labelKey: "contextMenu.colorGray" },
] as const;

const { t } = useI18n();

const {
  field: sortField,
  direction: sortDirection,
  setField: setSortField,
  setDirection: setSortDirection,
} = useSortPreference();

const {
  sidebarCollapsed,
  listCollapsed,
  sidebarWidth,
  listWidth,
  focusMode,
  toggleFocusMode,
  sidebarVisible,
  listVisible,
  gridTemplateColumns,
  startResize,
  stepResize,
  bounds: paneBounds,
} = usePaneLayout();
const {
  available: updateAvailable,
  version: updateVersion,
  installing: updateInstalling,
  checkForUpdate,
  installUpdate,
} = useAppUpdater();
const {
  isMaximized,
  minimize: minimizeWindow,
  toggleMaximize: toggleMaximizeWindow,
  close: closeWindow,
} = useWindowControls();
const { cyclePreference } = useTheme();

const loading = ref(true);
const notesRoot = ref<string | null>(null);

const folders = ref<Folder[]>([]);
const notes = ref<Note[]>([]);
const tags = ref<Tag[]>([]);

const selectedId = ref<string>("all");
const searchQuery = ref("");
const searchResults = ref<Note[] | null>(null);
const smartSearchSupported = ref(false);
const smartSearchEnabled = ref(false);
const selectedNoteId = ref<string | null>(null);
const editingBody = ref("");
const suppressAutosave = ref(false);
const sidebarRef = ref<InstanceType<typeof Sidebar> | null>(null);
const editorRef = ref<InstanceType<typeof Editor> | null>(null);

const contextMenu = ref<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
const promptModal = ref<{
  title: string;
  initialValue: string;
  confirmLabel: string;
  onConfirm: (value: string) => void;
} | null>(null);
const commandPaletteOpen = ref(false);
const toastMessage = ref<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | undefined;

function showToast(message: string) {
  toastMessage.value = message;
  if (toastTimer !== undefined) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toastMessage.value = null;
  }, 6000);
}

let unlistenNotesChanged: UnlistenFn | null = null;
let unlistenQuickCapture: UnlistenFn | null = null;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let searchTimer: ReturnType<typeof setTimeout> | undefined;
let smartSearchAvailabilityTimer: ReturnType<typeof setInterval> | undefined;
let dirty = false;

async function refreshData() {
  const [f, n, t] = await Promise.all([listFolders(), listNotes(), listTags()]);
  folders.value = f;
  notes.value = n;
  tags.value = t;
}

async function refreshSmartSearchAvailability() {
  const available = await smartSearchAvailable().catch(() => false);
  smartSearchSupported.value = available;
  if (!available) smartSearchEnabled.value = false;
}

onMounted(async () => {
  unlistenNotesChanged = await listen("notes-changed", () => {
    refreshData();
  });
  unlistenQuickCapture = await listen("quick-capture", () => {
    onQuickCapture();
  });

  notesRoot.value = await getNotesRoot();
  if (notesRoot.value) {
    await refreshData();
  }
  loading.value = false;

  await refreshSmartSearchAvailability();
  // Ollama can start, stop, or restart independently of the app (it's a
  // separate local service), so this can't be a one-shot check at launch:
  // poll periodically to pick up either transition without needing a
  // failed search or an app restart to notice.
  smartSearchAvailabilityTimer = setInterval(refreshSmartSearchAvailability, 15000);

  // Fire-and-forget: a slow/offline update check shouldn't hold up
  // startup, and a failure here is silent (see useAppUpdater) since
  // checking for updates is a nice-to-have, not a launch requirement.
  checkForUpdate();
});

onUnmounted(() => {
  unlistenNotesChanged?.();
  unlistenQuickCapture?.();
  if (smartSearchAvailabilityTimer !== undefined) clearInterval(smartSearchAvailabilityTimer);
});

async function onSetupReady() {
  notesRoot.value = await getNotesRoot();
  await refreshData();
}

const allCount = computed(
  () => notes.value.filter((n) => !n.deletedAt && !n.isTemplate).length,
);
const deletedCount = computed(() => notes.value.filter((n) => n.deletedAt).length);
const templateCount = computed(
  () => notes.value.filter((n) => n.isTemplate && !n.deletedAt).length,
);
const folderCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {};
  for (const folder of folders.value) {
    counts[folder.id] = notes.value.filter(
      (n) => n.folderId === folder.id && !n.deletedAt && !n.isTemplate,
    ).length;
  }
  return counts;
});

const isSearching = computed(() => searchQuery.value.trim().length > 0);

// While searching, filter/scope from the FTS5-ranked result set (full-text
// across title + body) instead of the plain in-memory list — same "scoped
// to the current folder/tag/trash view" behavior, better matching.
const searchPool = computed(() => (isSearching.value ? searchResults.value ?? [] : notes.value));

async function runSearch(trimmed: string) {
  if (smartSearchEnabled.value) {
    try {
      searchResults.value = await smartSearch(trimmed);
      return;
    } catch {
      // Ollama most likely stopped running: fall back to regular search
      // immediately rather than repeatedly failing on every keystroke.
      // The periodic availability check (see onMounted) will bring the
      // toggle back on its own once/if Ollama becomes reachable again.
      smartSearchSupported.value = false;
      smartSearchEnabled.value = false;
    }
  }
  searchResults.value = await searchNotes(trimmed);
}

watch(searchQuery, (q) => {
  if (searchTimer !== undefined) clearTimeout(searchTimer);
  const trimmed = q.trim();
  if (!trimmed) {
    searchResults.value = null;
    return;
  }
  // Smart Search embeds text via a local model on every call, so it gets
  // a longer debounce than plain FTS5 search to avoid firing on every
  // keystroke.
  searchTimer = setTimeout(() => runSearch(trimmed), smartSearchEnabled.value ? 400 : 150);
});

watch(smartSearchEnabled, () => {
  const trimmed = searchQuery.value.trim();
  if (trimmed) runSearch(trimmed);
});

const baseNotes = computed(() => {
  const pool = searchPool.value;
  if (selectedId.value === "recently-deleted") {
    return pool.filter((n) => n.deletedAt);
  }
  if (selectedId.value === "templates") {
    return pool.filter((n) => n.isTemplate && !n.deletedAt);
  }
  if (selectedId.value.startsWith("tag:")) {
    const tagId = selectedId.value.slice(4);
    return pool.filter((n) => !n.deletedAt && !n.isTemplate && n.tagIds.includes(tagId));
  }
  if (selectedId.value === "all") {
    return pool.filter((n) => !n.deletedAt && !n.isTemplate);
  }
  return pool.filter((n) => !n.deletedAt && !n.isTemplate && n.folderId === selectedId.value);
});

const showPinnedSections = computed(
  () =>
    selectedId.value !== "recently-deleted" &&
    selectedId.value !== "templates" &&
    !isSearching.value,
);

const canCreate = computed(
  () => selectedId.value !== "recently-deleted" && !selectedId.value.startsWith("tag:"),
);

// Where a newly created note (blank, or from a template) belongs: the
// current folder when one's selected, otherwise the root - "all"/
// "templates"/"recently-deleted"/a tag aren't real folder ids.
const currentFolderIdForCreate = computed(() => {
  const id = selectedId.value;
  if (id === "all" || id === "templates" || id === "recently-deleted" || id.startsWith("tag:")) {
    return "";
  }
  return id;
});

const filteredNotes = computed(() => {
  // Search results already come back relevance-ranked from FTS5; browsing
  // (not searching) sorts pinned-first then by the user's chosen field.
  if (isSearching.value) return baseNotes.value;
  const dir = sortDirection.value === "asc" ? 1 : -1;
  return [...baseNotes.value].sort((a, b) => {
    if (showPinnedSections.value && a.isPinned !== b.isPinned) {
      return a.isPinned ? -1 : 1;
    }
    if (sortField.value === "title") {
      return dir * a.title.localeCompare(b.title, undefined, { sensitivity: "base" });
    }
    const key = sortField.value;
    return dir * (new Date(a[key]).getTime() - new Date(b[key]).getTime());
  });
});

const listTitle = computed(() => {
  if (isSearching.value) {
    const prefix = smartSearchEnabled.value ? t("noteList.smartSearchPrefix") : "";
    return `${prefix}"${searchQuery.value.trim()}"`;
  }
  if (selectedId.value === "all") return t("common.allNotes");
  if (selectedId.value === "recently-deleted") return t("common.recentlyDeleted");
  if (selectedId.value === "templates") return t("common.templates");
  if (selectedId.value.startsWith("tag:")) {
    const tagId = selectedId.value.slice(4);
    return tags.value.find((tag) => tag.id === tagId)?.name ?? t("common.tag");
  }
  return folders.value.find((f) => f.id === selectedId.value)?.name ?? t("common.notes");
});

const selectedNote = computed(
  () => notes.value.find((n) => n.id === selectedNoteId.value) ?? null,
);

watch(
  filteredNotes,
  (list) => {
    if (!list.some((n) => n.id === selectedNoteId.value)) {
      selectedNoteId.value = list[0]?.id ?? null;
    }
  },
  { immediate: true },
);

async function flushSave(id: string) {
  if (saveTimer !== undefined) {
    clearTimeout(saveTimer);
    saveTimer = undefined;
  }
  if (!dirty) return;
  dirty = false;
  const result = await saveNoteBody(id, editingBody.value);
  if (result.outcome === "conflict") {
    showToast(
      t("editor.conflict.toast", { title: result.conflictedTitle || t("common.newNote") }),
    );
    // Only the note currently open needs its editor content replaced with
    // what's actually on disk now - a conflict on a note flushed while
    // switching away from it doesn't affect what's showing right now.
    if (selectedNoteId.value === id) {
      suppressAutosave.value = true;
      editingBody.value = result.originalBody;
      await nextTick();
      suppressAutosave.value = false;
    }
  }
  await refreshData();
}

function scheduleSave() {
  dirty = true;
  if (saveTimer !== undefined) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    if (selectedNoteId.value) flushSave(selectedNoteId.value);
  }, 500);
}

watch(editingBody, () => {
  if (suppressAutosave.value) return;
  scheduleSave();
});

watch(selectedNoteId, async (newId, oldId) => {
  if (oldId && dirty) {
    await flushSave(oldId);
  }
  suppressAutosave.value = true;
  editingBody.value = newId ? await getNoteBody(newId) : "";
  await nextTick();
  suppressAutosave.value = false;
});

async function onCreateNote() {
  const newId = await createNote(
    currentFolderIdForCreate.value,
    selectedId.value === "templates",
  );
  await refreshData();
  selectedNoteId.value = newId;
  editorRef.value?.focusEditor();
}

// Fired by the backend's global quick-capture hotkey, which may land while
// any sidebar section (or no notes root at all) is showing - switch to "All
// Notes" first so the blank note it creates is actually visible afterward.
async function onQuickCapture() {
  if (!notesRoot.value) return;
  selectedId.value = "all";
  await onCreateNote();
}

// "en-CA" is a reliable trick for YYYY-MM-DD: local (not UTC), so the
// daily note follows the user's own calendar day rather than flipping
// over at UTC midnight.
function todayLocalDate(): string {
  return new Date().toLocaleDateString("en-CA");
}

async function onOpenToday() {
  if (!notesRoot.value) return;
  const id = await getOrCreateDailyNote(todayLocalDate());
  await refreshData();
  selectedId.value = "all";
  selectedNoteId.value = id;
  editorRef.value?.focusEditor();
}

async function onImportMarkdown() {
  const dir = await openDialog({ directory: true, multiple: false });
  if (!dir || typeof dir !== "string") return;
  try {
    const count = await importMarkdownFolder(dir);
    await refreshData();
    showToast(count > 0 ? t("import.done", { count }) : t("import.noneFound"));
  } catch (e) {
    showToast(t("import.failed", { error: String(e) }));
  }
}

async function onExportBackup() {
  const path = await saveFileDialog({
    defaultPath: `${t("backup.defaultFilename", { date: todayLocalDate() })}.zip`,
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
  if (!path) return;
  try {
    await exportVaultBackup(path);
    showToast(t("backup.done"));
  } catch (e) {
    showToast(t("backup.failed", { error: String(e) }));
  }
}

async function onRestoreBackup() {
  const zipPath = await openDialog({
    multiple: false,
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
  if (!zipPath || typeof zipPath !== "string") return;

  const destDir = await openDialog({ directory: true, multiple: false });
  if (!destDir || typeof destDir !== "string") return;

  let count: number;
  try {
    count = await restoreVaultBackup(zipPath, destDir);
  } catch (e) {
    showToast(t("backup.restoreFailed", { error: String(e) }));
    return;
  }

  if (confirm(t("backup.restoreSwitchConfirm", { path: destDir }))) {
    await setNotesRoot(destDir);
    await onSetupReady();
    selectedId.value = "all";
    selectedNoteId.value = null;
  }
  showToast(t("backup.restoreDone", { count }));
}

const paletteActions = computed<PaletteAction[]>(() => [
  { id: "new-note", label: t("commandPalette.action.newNote"), icon: "plus", run: onCreateNote },
  { id: "today", label: t("commandPalette.action.today"), icon: "today", run: onOpenToday },
  {
    id: "import-markdown",
    label: t("import.button"),
    icon: "upload",
    run: onImportMarkdown,
  },
  {
    id: "export-backup",
    label: t("backup.button"),
    icon: "archive",
    run: onExportBackup,
  },
  {
    id: "restore-backup",
    label: t("backup.restoreButton"),
    icon: "restore",
    run: onRestoreBackup,
  },
  { id: "toggle-theme", label: t("commandPalette.action.toggleTheme"), icon: "monitor", run: cyclePreference },
  {
    id: "toggle-focus-mode",
    label: t("commandPalette.action.toggleFocusMode"),
    icon: "focus",
    run: toggleFocusMode,
  },
  {
    id: "toggle-sidebar",
    label: t("commandPalette.action.toggleSidebar"),
    icon: "panelLeft",
    run: () => (sidebarCollapsed.value = !sidebarCollapsed.value),
  },
  {
    id: "toggle-note-list",
    label: t("commandPalette.action.toggleNoteList"),
    icon: "panelList",
    run: () => (listCollapsed.value = !listCollapsed.value),
  },
  { id: "all-notes", label: t("commandPalette.action.allNotes"), icon: "notesList", run: () => (selectedId.value = "all") },
  {
    id: "templates",
    label: t("commandPalette.action.templates"),
    icon: "template",
    run: () => (selectedId.value = "templates"),
  },
  {
    id: "recently-deleted",
    label: t("commandPalette.action.recentlyDeleted"),
    icon: "trash",
    run: () => (selectedId.value = "recently-deleted"),
  },
]);

function onPaletteSelectNote(id: string) {
  selectedId.value = "all";
  selectedNoteId.value = id;
}

function onPaletteSelectFolder(id: string) {
  selectedId.value = id;
}

function onPaletteSelectTag(id: string) {
  selectedId.value = `tag:${id}`;
}

async function onCreateFromTemplate(templateId: string) {
  const newId = await createNoteFromTemplate(currentFolderIdForCreate.value, templateId);
  await refreshData();
  selectedNoteId.value = newId;
  editorRef.value?.focusEditor();
}

function onCreateContextmenu(event: MouseEvent) {
  const templates = notes.value.filter((n) => n.isTemplate && !n.deletedAt);
  const items: ContextMenuItem[] = [{ label: t("noteList.blankNote"), action: onCreateNote }];
  if (templates.length > 0) {
    items.push({ label: "", action: () => {}, separator: true });
    for (const template of templates) {
      items.push({
        label: template.title || t("common.newNote"),
        action: () => onCreateFromTemplate(template.id),
      });
    }
  }
  contextMenu.value = { x: event.clientX, y: event.clientY, items };
}

async function onTogglePin() {
  if (!selectedNote.value) return;
  await setNotePinned(selectedNote.value.id, !selectedNote.value.isPinned);
  await refreshData();
}

async function onToggleDeleted() {
  if (!selectedNote.value) return;
  await setNoteDeleted(selectedNote.value.id, !selectedNote.value.deletedAt);
  await refreshData();
}

// Switches to "All Notes" so the target is visible/highlighted in the list
// regardless of which folder or tag was previously selected - a [[link]]
// can point anywhere, not just within the current scope.
function onNavigateToNote(id: string) {
  selectedId.value = "all";
  selectedNoteId.value = id;
}

function onAddTag() {
  if (!selectedNote.value) return;
  const noteId = selectedNote.value.id;
  promptModal.value = {
    title: t("promptModal.addTagTitle"),
    initialValue: "",
    confirmLabel: t("common.add"),
    onConfirm: async (name) => {
      await addNoteTag(noteId, name);
      await refreshData();
      promptModal.value = null;
    },
  };
}

async function onRemoveTag(name: string) {
  if (!selectedNote.value) return;
  await removeNoteTag(selectedNote.value.id, name);
  await refreshData();
}

function closeContextMenu() {
  contextMenu.value = null;
}

function onNoteContextMenu(event: MouseEvent, note: Note) {
  const items: ContextMenuItem[] = [];

  if (note.deletedAt) {
    items.push({ label: t("common.restore"), action: () => setNoteDeleted(note.id, false).then(refreshData) });
    items.push({
      label: t("contextMenu.deletePermanently"),
      danger: true,
      action: () => {
        const title = note.title || t("common.newNote");
        if (confirm(t("contextMenu.confirmDeletePermanent", { title }))) {
          deleteNotePermanently(note.id).then(refreshData);
        }
      },
    });
  } else {
    items.push({
      label: note.isPinned ? t("common.unpin") : t("common.pin"),
      action: () => setNotePinned(note.id, !note.isPinned).then(refreshData),
    });
    items.push({
      label: note.isTemplate
        ? t("contextMenu.removeFromTemplates")
        : t("contextMenu.saveAsTemplate"),
      action: () => setNoteTemplate(note.id, !note.isTemplate).then(refreshData),
    });

    const otherFolders = folders.value.filter((f) => f.id !== note.folderId);
    if (otherFolders.length > 0) {
      items.push({ label: "", action: () => {}, separator: true });
      for (const folder of otherFolders) {
        items.push({
          label: t("contextMenu.moveTo", { folder: folder.name }),
          action: () => moveNote(note.id, folder.id).then(refreshData),
        });
      }
    }

    items.push({ label: "", action: () => {}, separator: true });
    items.push({
      label: t("common.delete"),
      danger: true,
      action: () => setNoteDeleted(note.id, true).then(refreshData),
    });
  }

  contextMenu.value = { x: event.clientX, y: event.clientY, items };
}

function onFolderContextmenu(event: MouseEvent, folder: Folder) {
  const isRoot = folder.id === "";
  const items: ContextMenuItem[] = [
    {
      label: t("contextMenu.newSubfolder"),
      action: () => promptNewFolder(folder.id),
    },
    {
      label: t("contextMenu.renameEllipsis"),
      disabled: isRoot,
      action: () => promptRenameFolder(folder),
    },
    { label: "", action: () => {}, separator: true },
    {
      label: "",
      action: () => {},
      swatches: [
        {
          color: null,
          label: t("contextMenu.colorNone"),
          selected: !folder.color,
          action: () => setFolderColor(folder.id, null).then(refreshData),
        },
        ...COLOR_PALETTE.map(({ value, labelKey }) => ({
          color: value,
          label: t(labelKey),
          selected: folder.color === value,
          action: () => setFolderColor(folder.id, value).then(refreshData),
        })),
      ],
    },
    {
      label: t("common.delete"),
      danger: true,
      disabled: isRoot,
      action: async () => {
        try {
          await deleteFolder(folder.id);
          if (selectedId.value === folder.id) selectedId.value = "all";
          await refreshData();
        } catch (e) {
          alert(t("contextMenu.cantDeleteFolder", { name: folder.name, error: String(e) }));
        }
      },
    },
  ];
  contextMenu.value = { x: event.clientX, y: event.clientY, items };
}

function onTagContextmenu(event: MouseEvent, tag: Tag) {
  const items: ContextMenuItem[] = [
    {
      label: "",
      action: () => {},
      swatches: [
        {
          color: null,
          label: t("contextMenu.colorNone"),
          selected: !tag.color,
          action: () => setTagColor(tag.id, null).then(refreshData),
        },
        ...COLOR_PALETTE.map(({ value, labelKey }) => ({
          color: value,
          label: t(labelKey),
          selected: tag.color === value,
          action: () => setTagColor(tag.id, value).then(refreshData),
        })),
      ],
    },
  ];
  contextMenu.value = { x: event.clientX, y: event.clientY, items };
}

function onSortClick(event: MouseEvent) {
  const items: ContextMenuItem[] = [
    {
      label: t("noteList.sort.dateModified"),
      checked: sortField.value === "updatedAt",
      action: () => setSortField("updatedAt"),
    },
    {
      label: t("noteList.sort.dateCreated"),
      checked: sortField.value === "createdAt",
      action: () => setSortField("createdAt"),
    },
    {
      label: t("noteList.sort.title"),
      checked: sortField.value === "title",
      action: () => setSortField("title"),
    },
    { label: "", action: () => {}, separator: true },
    {
      label: t("noteList.sort.ascending"),
      checked: sortDirection.value === "asc",
      action: () => setSortDirection("asc"),
    },
    {
      label: t("noteList.sort.descending"),
      checked: sortDirection.value === "desc",
      action: () => setSortDirection("desc"),
    },
  ];
  contextMenu.value = { x: event.clientX, y: event.clientY, items };
}

function promptNewFolder(parentId: string) {
  promptModal.value = {
    title: t("promptModal.newFolderTitle"),
    initialValue: "",
    confirmLabel: t("common.create"),
    onConfirm: async (name) => {
      const id = await createFolder(parentId, name);
      await refreshData();
      selectedId.value = id;
      promptModal.value = null;
    },
  };
}

function promptRenameFolder(folder: Folder) {
  promptModal.value = {
    title: t("promptModal.renameFolderTitle"),
    initialValue: folder.name,
    confirmLabel: t("common.rename"),
    onConfirm: async (name) => {
      const newId = await renameFolder(folder.id, name);
      if (selectedId.value === folder.id) selectedId.value = newId;
      await refreshData();
      promptModal.value = null;
    },
  };
}

function isTypingContext(): boolean {
  const el = document.activeElement as HTMLElement | null;
  if (!el) return false;
  return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
}

// Ctrl/Cmd+F is contextual: from anywhere in the editor pane (the content
// itself, its toolbar, the find bar) it means "find in this note"; from
// everywhere else (sidebar, note list) it means the existing global search.
function isEditorFocused(): boolean {
  const el = document.activeElement as HTMLElement | null;
  return !!el?.closest(".editor");
}

function navigateList(direction: number) {
  const list = filteredNotes.value;
  if (list.length === 0) return;
  const currentIndex = list.findIndex((n) => n.id === selectedNoteId.value);
  const nextIndex =
    currentIndex === -1 ? 0 : Math.min(Math.max(currentIndex + direction, 0), list.length - 1);
  selectedNoteId.value = list[nextIndex].id;
}

function onGlobalKeydown(e: KeyboardEvent) {
  // While a context menu or dialog is open, its own keydown handler owns
  // the keyboard (WAI-ARIA menu/dialog patterns) - without this guard,
  // e.g. arrow keys meant to move through the menu also bubble up here
  // and silently change the selected note underneath it.
  if (contextMenu.value || promptModal.value || commandPaletteOpen.value) return;

  const mod = e.ctrlKey || e.metaKey;

  if (mod && e.key.toLowerCase() === "n") {
    e.preventDefault();
    if (canCreate.value) onCreateNote();
    return;
  }

  if (mod && e.key.toLowerCase() === "f") {
    e.preventDefault();
    if (selectedNote.value && isEditorFocused()) {
      editorRef.value?.openFind();
      return;
    }
    if (sidebarCollapsed.value) {
      sidebarCollapsed.value = false;
      nextTick(() => sidebarRef.value?.focusSearch());
    } else {
      sidebarRef.value?.focusSearch();
    }
    return;
  }

  if (mod && e.key === ".") {
    e.preventDefault();
    toggleFocusMode();
    return;
  }

  if (mod && e.key.toLowerCase() === "t") {
    e.preventDefault();
    onOpenToday();
    return;
  }

  if (mod && e.key.toLowerCase() === "k") {
    e.preventDefault();
    commandPaletteOpen.value = true;
    return;
  }

  if (!mod && (e.key === "ArrowDown" || e.key === "ArrowUp") && !isTypingContext()) {
    e.preventDefault();
    navigateList(e.key === "ArrowDown" ? 1 : -1);
  }
}

onMounted(() => window.addEventListener("keydown", onGlobalKeydown));
onUnmounted(() => window.removeEventListener("keydown", onGlobalKeydown));
</script>

<template>
  <FirstRunSetup v-if="!loading && !notesRoot" @ready="onSetupReady" />

  <div v-else-if="!loading" class="app-root">
    <div class="top-bar" data-tauri-drag-region="deep">
      <div class="top-bar-start">
        <button
          class="pane-toggle"
          :class="{ active: !sidebarCollapsed }"
          :title="t('topBar.toggleSidebar')"
          :aria-label="t('topBar.toggleSidebar')"
          :aria-pressed="!sidebarCollapsed"
          @click="sidebarCollapsed = !sidebarCollapsed"
        >
          <Icon name="panelLeft" />
        </button>
        <button
          class="pane-toggle"
          :class="{ active: !listCollapsed }"
          :title="t('topBar.toggleNoteList')"
          :aria-label="t('topBar.toggleNoteList')"
          :aria-pressed="!listCollapsed"
          @click="listCollapsed = !listCollapsed"
        >
          <Icon name="panelList" />
        </button>
        <button
          class="pane-toggle"
          :class="{ active: focusMode }"
          :title="t('topBar.toggleFocusMode')"
          :aria-label="t('topBar.toggleFocusMode')"
          :aria-pressed="focusMode"
          @click="toggleFocusMode"
        >
          <Icon name="focus" />
        </button>
      </div>

      <div class="top-bar-end">
        <button
          v-if="updateAvailable"
          class="update-available"
          :disabled="updateInstalling"
          :title="t('topBar.installUpdate', { version: updateVersion })"
          @click="installUpdate"
        >
          <Icon name="download" />
          <span>{{ updateInstalling ? t('topBar.installing') : t('topBar.updateTo', { version: updateVersion }) }}</span>
        </button>

        <div class="window-controls">
          <button
            class="window-control-button"
            :title="t('topBar.minimize')"
            :aria-label="t('topBar.minimize')"
            @click="minimizeWindow"
          >
            <Icon name="windowMinimize" />
          </button>
          <button
            class="window-control-button"
            :title="isMaximized ? t('topBar.restore') : t('topBar.maximize')"
            :aria-label="isMaximized ? t('topBar.restore') : t('topBar.maximize')"
            @click="toggleMaximizeWindow"
          >
            <Icon :name="isMaximized ? 'windowRestore' : 'windowMaximize'" />
          </button>
          <button
            class="window-control-button window-control-close"
            :title="t('topBar.close')"
            :aria-label="t('topBar.close')"
            @click="closeWindow"
          >
            <Icon name="close" />
          </button>
        </div>
      </div>
    </div>

    <div class="app-shell" :style="{ gridTemplateColumns }">
      <Sidebar
        v-if="sidebarVisible"
        ref="sidebarRef"
        v-model:selected-id="selectedId"
        v-model:search-query="searchQuery"
        v-model:smart-search-enabled="smartSearchEnabled"
        style="grid-column: 1"
        :folders="folders"
        :tags="tags"
        :all-count="allCount"
        :deleted-count="deletedCount"
        :template-count="templateCount"
        :folder-counts="folderCounts"
        :smart-search-supported="smartSearchSupported"
        @new-folder="promptNewFolder('')"
        @open-today="onOpenToday"
        @import-markdown="onImportMarkdown"
        @export-backup="onExportBackup"
        @restore-backup="onRestoreBackup"
        @folder-contextmenu="onFolderContextmenu"
        @tag-contextmenu="onTagContextmenu"
      />
      <div
        v-if="sidebarVisible"
        class="resize-handle"
        style="grid-column: 2"
        role="separator"
        aria-orientation="vertical"
        :aria-label="t('resize.sidebar')"
        :aria-valuenow="sidebarWidth"
        :aria-valuemin="paneBounds.sidebar.min"
        :aria-valuemax="paneBounds.sidebar.max"
        tabindex="0"
        @pointerdown="startResize('sidebar', $event)"
        @keydown.left="stepResize('sidebar', -10)"
        @keydown.right="stepResize('sidebar', 10)"
      />

      <NoteList
        v-if="listVisible && selectedId !== 'graph'"
        v-model:selected-id="selectedNoteId"
        style="grid-column: 3"
        :notes="filteredNotes"
        :title="listTitle"
        :show-pinned-sections="showPinnedSections"
        :can-create="canCreate"
        :is-trash="selectedId === 'recently-deleted'"
        :is-templates="selectedId === 'templates'"
        @create="onCreateNote"
        @create-contextmenu="onCreateContextmenu"
        @contextmenu="onNoteContextMenu"
        @sort-click="onSortClick"
      />
      <div
        v-if="listVisible && selectedId !== 'graph'"
        class="resize-handle"
        style="grid-column: 4"
        role="separator"
        aria-orientation="vertical"
        :aria-label="t('resize.noteList')"
        :aria-valuenow="listWidth"
        :aria-valuemin="paneBounds.list.min"
        :aria-valuemax="paneBounds.list.max"
        tabindex="0"
        @pointerdown="startResize('list', $event)"
        @keydown.left="stepResize('list', -10)"
        @keydown.right="stepResize('list', 10)"
      />

      <GraphView
        v-if="selectedId === 'graph'"
        style="grid-column: 3 / -1"
        :notes="notes"
        @select-note="onPaletteSelectNote"
      />

      <Editor
        v-if="selectedId !== 'graph'"
        ref="editorRef"
        v-model:body="editingBody"
        style="grid-column: 5"
        :note="selectedNote"
        :notes="notes"
        :folders="folders"
        :tags="tags"
        :smart-search-supported="smartSearchSupported"
        @toggle-pin="onTogglePin"
        @toggle-deleted="onToggleDeleted"
        @add-tag="onAddTag"
        @remove-tag="onRemoveTag"
        @navigate-to-note="onNavigateToNote"
      />
    </div>

    <ContextMenu
      v-if="contextMenu"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :items="contextMenu.items"
      @close="closeContextMenu"
    />
    <PromptModal
      v-if="promptModal"
      :title="promptModal.title"
      :initial-value="promptModal.initialValue"
      :confirm-label="promptModal.confirmLabel"
      @confirm="promptModal.onConfirm"
      @cancel="promptModal = null"
    />
    <CommandPalette
      v-if="commandPaletteOpen"
      :notes="notes"
      :folders="folders"
      :tags="tags"
      :actions="paletteActions"
      @close="commandPaletteOpen = false"
      @select-note="onPaletteSelectNote"
      @select-folder="onPaletteSelectFolder"
      @select-tag="onPaletteSelectTag"
    />
    <Teleport to="body">
      <div v-if="toastMessage" class="toast" role="status">{{ toastMessage }}</div>
    </Teleport>
  </div>
</template>

<style scoped>
.app-root {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100%;
  overflow: hidden;
}

.top-bar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 5px 5px 5px 8px;
  background: var(--bg-sidebar);
  border-bottom: 1px solid var(--border);
  /* A native-feeling drag handle needs some height to grab - the row's
     content alone (26px buttons) reads as too thin a target. */
  min-height: 38px;
  box-sizing: border-box;
}

.top-bar-start,
.top-bar-end {
  display: flex;
  align-items: center;
  gap: 2px;
}

.pane-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
}

.pane-toggle:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.update-available {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px 5px 8px;
  border: none;
  border-radius: 999px;
  /* Solid per-theme colors rather than a translucent accent-blue tint:
     the tint composited too pale (light) / too dark (dark) for either a
     single text color or --accent-blue itself to clear 4.5:1 against. */
  background: var(--pill-blue-bg);
  color: var(--pill-blue-text);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
}

.update-available svg {
  width: 14px;
  height: 14px;
}

.update-available:hover:not(:disabled) {
  filter: brightness(0.95);
}

.update-available:disabled {
  cursor: default;
  opacity: 0.7;
}

.toast {
  position: fixed;
  left: 50%;
  bottom: 28px;
  transform: translateX(-50%);
  max-width: min(480px, calc(100vw - 32px));
  padding: 10px 16px;
  border-radius: 10px;
  background: var(--bg-selected-strong);
  color: #fff;
  font-size: 13px;
  text-align: center;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
  z-index: 1000;
}

.pane-toggle.active {
  color: var(--accent-blue);
}

.pane-toggle svg {
  width: 16px;
  height: 16px;
}

.window-controls {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-left: 6px;
  padding-left: 8px;
  border-left: 1px solid var(--border);
}

.window-control-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}

.window-control-button svg {
  width: 13px;
  height: 13px;
}

.window-control-button:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.window-control-button:active {
  filter: brightness(0.92);
}

.window-control-close:hover {
  background: #e81123;
  color: #fff;
}

.app-shell {
  flex: 1 1 auto;
  display: grid;
  overflow: hidden;
  min-height: 0;
}

.resize-handle {
  cursor: col-resize;
  background: transparent;
}

.resize-handle:hover,
.resize-handle:active,
.resize-handle:focus-visible {
  background: var(--accent-blue);
  opacity: 0.5;
}

.resize-handle:focus-visible {
  outline: 2px solid var(--accent-blue);
  outline-offset: -2px;
}
</style>
