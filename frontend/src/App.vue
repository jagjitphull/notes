<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import Sidebar from "./components/Sidebar.vue";
import NoteList from "./components/NoteList.vue";
import Editor from "./components/Editor.vue";
import FirstRunSetup from "./components/FirstRunSetup.vue";
import ContextMenu, { type ContextMenuItem } from "./components/ContextMenu.vue";
import PromptModal from "./components/PromptModal.vue";
import {
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
  renameFolder,
  saveNoteBody,
  searchNotes,
  setNoteDeleted,
  setNotePinned,
} from "./api";
import type { Folder, Note, Tag } from "./types";

const loading = ref(true);
const notesRoot = ref<string | null>(null);

const folders = ref<Folder[]>([]);
const notes = ref<Note[]>([]);
const tags = ref<Tag[]>([]);

const selectedId = ref<string>("all");
const searchQuery = ref("");
const searchResults = ref<Note[] | null>(null);
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
let dirty = false;

async function refreshData() {
  const [f, n, t] = await Promise.all([listFolders(), listNotes(), listTags()]);
  folders.value = f;
  notes.value = n;
  tags.value = t;
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
});

onUnmounted(() => {
  unlistenNotesChanged?.();
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

watch(searchQuery, (q) => {
  if (searchTimer !== undefined) clearTimeout(searchTimer);
  const trimmed = q.trim();
  if (!trimmed) {
    searchResults.value = null;
    return;
  }
  searchTimer = setTimeout(async () => {
    searchResults.value = await searchNotes(trimmed);
  }, 150);
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
  // (not searching) sorts pinned-first then by modified date instead.
  if (isSearching.value) return baseNotes.value;
  return [...baseNotes.value].sort((a, b) => {
    if (showPinnedSections.value && a.isPinned !== b.isPinned) {
      return a.isPinned ? -1 : 1;
    }
    return new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime();
  });
});

const listTitle = computed(() => {
  if (isSearching.value) return `"${searchQuery.value.trim()}"`;
  if (selectedId.value === "all") return "All Notes";
  if (selectedId.value === "recently-deleted") return "Recently Deleted";
  if (selectedId.value.startsWith("tag:")) {
    const tagId = selectedId.value.slice(4);
    return tags.value.find((t) => t.id === tagId)?.name ?? "Tag";
  }
  return folders.value.find((f) => f.id === selectedId.value)?.name ?? "Notes";
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

function closeContextMenu() {
  contextMenu.value = null;
}

function onNoteContextMenu(event: MouseEvent, note: Note) {
  const items: ContextMenuItem[] = [];

  if (note.deletedAt) {
    items.push({ label: "Restore", action: () => setNoteDeleted(note.id, false).then(refreshData) });
    items.push({
      label: "Delete Permanently",
      danger: true,
      action: () => {
        if (confirm(`Permanently delete "${note.title || "New Note"}"? This can't be undone.`)) {
          deleteNotePermanently(note.id).then(refreshData);
        }
      },
    });
  } else {
    items.push({
      label: note.isPinned ? "Unpin" : "Pin",
      action: () => setNotePinned(note.id, !note.isPinned).then(refreshData),
    });

    const otherFolders = folders.value.filter((f) => f.id !== note.folderId);
    if (otherFolders.length > 0) {
      items.push({ label: "", action: () => {}, separator: true });
      for (const folder of otherFolders) {
        items.push({
          label: `Move to “${folder.name}”`,
          action: () => moveNote(note.id, folder.id).then(refreshData),
        });
      }
    }

    items.push({ label: "", action: () => {}, separator: true });
    items.push({
      label: "Delete",
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
      label: "New Subfolder…",
      action: () => promptNewFolder(folder.id),
    },
    {
      label: "Rename…",
      disabled: isRoot,
      action: () => promptRenameFolder(folder),
    },
    {
      label: "Delete",
      danger: true,
      disabled: isRoot,
      action: async () => {
        try {
          await deleteFolder(folder.id);
          if (selectedId.value === folder.id) selectedId.value = "all";
          await refreshData();
        } catch (e) {
          alert(`Can't delete "${folder.name}": ${e}`);
        }
      },
    },
  ];
  contextMenu.value = { x: event.clientX, y: event.clientY, items };
}

function promptNewFolder(parentId: string) {
  promptModal.value = {
    title: "New Folder",
    initialValue: "",
    confirmLabel: "Create",
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
    title: "Rename Folder",
    initialValue: folder.name,
    confirmLabel: "Rename",
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

function navigateList(direction: number) {
  const list = filteredNotes.value;
  if (list.length === 0) return;
  const currentIndex = list.findIndex((n) => n.id === selectedNoteId.value);
  const nextIndex =
    currentIndex === -1 ? 0 : Math.min(Math.max(currentIndex + direction, 0), list.length - 1);
  selectedNoteId.value = list[nextIndex].id;
}

function onGlobalKeydown(e: KeyboardEvent) {
  const mod = e.ctrlKey || e.metaKey;

  if (mod && e.key.toLowerCase() === "n") {
    e.preventDefault();
    if (canCreate.value) onCreateNote();
    return;
  }

  if (mod && e.key.toLowerCase() === "f") {
    e.preventDefault();
    sidebarRef.value?.focusSearch();
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

  <div v-else-if="!loading" class="app-shell">
    <Sidebar
      ref="sidebarRef"
      v-model:selected-id="selectedId"
      v-model:search-query="searchQuery"
      :folders="folders"
      :tags="tags"
      :all-count="allCount"
      :deleted-count="deletedCount"
      :folder-counts="folderCounts"
      @new-folder="promptNewFolder('')"
      @folder-contextmenu="onFolderContextmenu"
    />
    <NoteList
      v-model:selected-id="selectedNoteId"
      :notes="filteredNotes"
      :title="listTitle"
      :show-pinned-sections="showPinnedSections"
      :can-create="canCreate"
      :is-trash="selectedId === 'recently-deleted'"
      @create="onCreateNote"
      @contextmenu="onNoteContextMenu"
    />
    <Editor
      ref="editorRef"
      v-model:body="editingBody"
      :note="selectedNote"
      :folders="folders"
      @toggle-pin="onTogglePin"
      @toggle-deleted="onToggleDeleted"
    />

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
.app-shell {
  display: grid;
  grid-template-columns: 220px 300px 1fr;
  height: 100vh;
  width: 100%;
  overflow: hidden;
}
</style>
