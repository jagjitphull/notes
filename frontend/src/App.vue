<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import Icon from "./components/icons/Icon.vue";
import Sidebar from "./components/Sidebar.vue";
import NoteList from "./components/NoteList.vue";
import Editor from "./components/Editor.vue";
import FirstRunSetup from "./components/FirstRunSetup.vue";
import ContextMenu, { type ContextMenuItem } from "./components/ContextMenu.vue";
import PromptModal from "./components/PromptModal.vue";
import {
  addNoteTag,
  createFolder,
  createNote,
  deleteFolder,
  deleteNotePermanently,
  getNoteBody,
  getNotesRoot,
  listFolders,
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
  smartSearch,
  smartSearchAvailable,
} from "./api";
import { useAppUpdater } from "./composables/useAppUpdater";
import { usePaneLayout } from "./composables/usePaneLayout";
import { useSortPreference } from "./composables/useSortPreference";
import type { Folder, Note, Tag } from "./types";

// A fixed accent palette (matching the system-color style Apple Notes/
// Finder use for tags and folders) rather than a free-form color picker -
// keeps every folder's tint one of a small, visually distinct set.
const FOLDER_COLOR_PALETTE = [
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

let unlistenNotesChanged: UnlistenFn | null = null;
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
  if (smartSearchAvailabilityTimer !== undefined) clearInterval(smartSearchAvailabilityTimer);
});

async function onSetupReady() {
  notesRoot.value = await getNotesRoot();
  await refreshData();
}

const allCount = computed(() => notes.value.filter((n) => !n.deletedAt).length);
const deletedCount = computed(() => notes.value.filter((n) => n.deletedAt).length);
const folderCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {};
  for (const folder of folders.value) {
    counts[folder.id] = notes.value.filter(
      (n) => n.folderId === folder.id && !n.deletedAt,
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
  if (selectedId.value.startsWith("tag:")) {
    const tagId = selectedId.value.slice(4);
    return pool.filter((n) => !n.deletedAt && n.tagIds.includes(tagId));
  }
  if (selectedId.value === "all") {
    return pool.filter((n) => !n.deletedAt);
  }
  return pool.filter((n) => !n.deletedAt && n.folderId === selectedId.value);
});

const showPinnedSections = computed(
  () => selectedId.value !== "recently-deleted" && !isSearching.value,
);

const canCreate = computed(
  () => selectedId.value !== "recently-deleted" && !selectedId.value.startsWith("tag:"),
);

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
  await saveNoteBody(id, editingBody.value);
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
  const folderId = selectedId.value === "all" ? "" : selectedId.value;
  const newId = await createNote(folderId);
  await refreshData();
  selectedNoteId.value = newId;
  editorRef.value?.focusEditor();
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
          label: t("contextMenu.folderColorNone"),
          selected: !folder.color,
          action: () => setFolderColor(folder.id, null).then(refreshData),
        },
        ...FOLDER_COLOR_PALETTE.map(({ value, labelKey }) => ({
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
  if (contextMenu.value || promptModal.value) return;

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
    <div class="top-bar">
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
        v-if="updateAvailable"
        class="update-available"
        :disabled="updateInstalling"
        :title="t('topBar.installUpdate', { version: updateVersion })"
        @click="installUpdate"
      >
        <Icon name="download" />
        <span>{{ updateInstalling ? t('topBar.installing') : t('topBar.updateTo', { version: updateVersion }) }}</span>
      </button>
    </div>

    <div class="app-shell" :style="{ gridTemplateColumns }">
      <Sidebar
        v-if="!sidebarCollapsed"
        ref="sidebarRef"
        v-model:selected-id="selectedId"
        v-model:search-query="searchQuery"
        v-model:smart-search-enabled="smartSearchEnabled"
        style="grid-column: 1"
        :folders="folders"
        :tags="tags"
        :all-count="allCount"
        :deleted-count="deletedCount"
        :folder-counts="folderCounts"
        :smart-search-supported="smartSearchSupported"
        @new-folder="promptNewFolder('')"
        @folder-contextmenu="onFolderContextmenu"
      />
      <div
        v-if="!sidebarCollapsed"
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
        v-if="!listCollapsed"
        v-model:selected-id="selectedNoteId"
        style="grid-column: 3"
        :notes="filteredNotes"
        :title="listTitle"
        :show-pinned-sections="showPinnedSections"
        :can-create="canCreate"
        :is-trash="selectedId === 'recently-deleted'"
        @create="onCreateNote"
        @contextmenu="onNoteContextMenu"
        @sort-click="onSortClick"
      />
      <div
        v-if="!listCollapsed"
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

      <Editor
        ref="editorRef"
        v-model:body="editingBody"
        style="grid-column: 5"
        :note="selectedNote"
        :notes="notes"
        :folders="folders"
        :tags="tags"
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
  gap: 2px;
  padding: 5px 8px;
  background: var(--bg-sidebar);
  border-bottom: 1px solid var(--border);
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
  margin-left: auto;
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

.pane-toggle.active {
  color: var(--accent-blue);
}

.pane-toggle svg {
  width: 16px;
  height: 16px;
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
