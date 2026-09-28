<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Sidebar from "./components/Sidebar.vue";
import NoteList from "./components/NoteList.vue";
import Editor from "./components/Editor.vue";
import { folders, notes, tags } from "./data/mockData";

const selectedId = ref<string>("all");
const searchQuery = ref("");
const selectedNoteId = ref<string | null>(null);

const allCount = computed(() => notes.filter((n) => !n.deletedAt).length);
const deletedCount = computed(() => notes.filter((n) => n.deletedAt).length);
const folderCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {};
  for (const folder of folders) {
    counts[folder.id] = notes.filter(
      (n) => n.folderId === folder.id && !n.deletedAt,
    ).length;
  }
  return counts;
});

const isSearching = computed(() => searchQuery.value.trim().length > 0);

const baseNotes = computed(() => {
  if (selectedId.value === "recently-deleted") {
    return notes.filter((n) => n.deletedAt);
  }
  if (selectedId.value.startsWith("tag:")) {
    const tagId = selectedId.value.slice(4);
    return notes.filter((n) => !n.deletedAt && n.tagIds.includes(tagId));
  }
  if (selectedId.value === "all") {
    return notes.filter((n) => !n.deletedAt);
  }
  return notes.filter((n) => !n.deletedAt && n.folderId === selectedId.value);
});

const showPinnedSections = computed(
  () => selectedId.value !== "recently-deleted" && !isSearching.value,
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
    return (
      new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
    );
  });
});

const listTitle = computed(() => {
  if (isSearching.value) return `"${searchQuery.value.trim()}"`;
  if (selectedId.value === "all") return "All Notes";
  if (selectedId.value === "recently-deleted") return "Recently Deleted";
  if (selectedId.value.startsWith("tag:")) {
    const tagId = selectedId.value.slice(4);
    return tags.find((t) => t.id === tagId)?.name ?? "Tag";
  }
  return folders.find((f) => f.id === selectedId.value)?.name ?? "Notes";
});

const selectedNote = computed(
  () => notes.find((n) => n.id === selectedNoteId.value) ?? null,
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
</script>

<template>
  <div class="app-shell">
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
    />
    <Editor :note="selectedNote" :folders="folders" />
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
