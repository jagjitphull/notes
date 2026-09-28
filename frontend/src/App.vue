<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import Sidebar from "./components/Sidebar.vue";
import NoteList from "./components/NoteList.vue";
import Editor from "./components/Editor.vue";
import FirstRunSetup from "./components/FirstRunSetup.vue";
import {
  createNote,
  getNoteBody,
  getNotesRoot,
  listFolders,
  listNotes,
  listTags,
  saveNoteBody,
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
const selectedNoteId = ref<string | null>(null);
const editingBody = ref("");
const suppressAutosave = ref(false);

let unlistenNotesChanged: UnlistenFn | null = null;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
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

const baseNotes = computed(() => {
  if (selectedId.value === "recently-deleted") {
    return notes.value.filter((n) => n.deletedAt);
  }
  if (selectedId.value.startsWith("tag:")) {
    const tagId = selectedId.value.slice(4);
    return notes.value.filter((n) => !n.deletedAt && n.tagIds.includes(tagId));
  }
  if (selectedId.value === "all") {
    return notes.value.filter((n) => !n.deletedAt);
  }
  return notes.value.filter((n) => !n.deletedAt && n.folderId === selectedId.value);
});

const showPinnedSections = computed(
  () => selectedId.value !== "recently-deleted" && !isSearching.value,
);

const canCreate = computed(
  () => selectedId.value !== "recently-deleted" && !selectedId.value.startsWith("tag:"),
);

const filteredNotes = computed(() => {
  let result = baseNotes.value;
  if (isSearching.value) {
    const q = searchQuery.value.trim().toLowerCase();
    result = result.filter(
      (n) =>
        n.title.toLowerCase().includes(q) ||
        n.plaintextContent.toLowerCase().includes(q),
    );
  }
  return [...result].sort((a, b) => {
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
</script>

<template>
  <FirstRunSetup v-if="!loading && !notesRoot" @ready="onSetupReady" />

  <div v-else-if="!loading" class="app-shell">
    <Sidebar
      v-model:selected-id="selectedId"
      v-model:search-query="searchQuery"
      :folders="folders"
      :tags="tags"
      :all-count="allCount"
      :deleted-count="deletedCount"
      :folder-counts="folderCounts"
    />
    <NoteList
      v-model:selected-id="selectedNoteId"
      :notes="filteredNotes"
      :title="listTitle"
      :show-pinned-sections="showPinnedSections"
      :can-create="canCreate"
      @create="onCreateNote"
    />
    <Editor
      v-model:body="editingBody"
      :note="selectedNote"
      :folders="folders"
      @toggle-pin="onTogglePin"
      @toggle-deleted="onToggleDeleted"
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
