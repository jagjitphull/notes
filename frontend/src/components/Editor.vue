<script setup lang="ts">
import { computed } from "vue";
import type { Folder, Note } from "../types";

const props = defineProps<{
  note: Note | null;
  folders: Folder[];
}>();

const folderName = computed(
  () => props.folders.find((f) => f.id === props.note?.folderId)?.name ?? "",
);

const formattedDate = computed(() => {
  if (!props.note) return "";
  return new Date(props.note.updatedAt).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
});
</script>

<template>
  <main class="editor">
    <template v-if="note">
      <div class="editor-toolbar">
        <span class="editor-meta">{{ formattedDate }} &middot; {{ folderName }}</span>
      </div>
      <div class="editor-canvas">
        <h1 class="note-title">{{ note.title || "New Note" }}</h1>
        <p
          v-for="(line, i) in note.plaintextContent.split('\n')"
          :key="i"
          class="note-line"
        >
          {{ line || " " }}
        </p>
      </div>
    </template>

    <div v-else class="empty-editor">
      <svg viewBox="0 0 48 48" fill="none" aria-hidden="true">
        <rect x="8" y="6" width="32" height="36" rx="4" stroke="currentColor" stroke-width="2" />
        <path d="M15 16h18M15 23h18M15 30h11" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
      </svg>
      <p>No Note Selected</p>
    </div>
  </main>
</template>

<style scoped>
.editor {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-editor);
}

.editor-toolbar {
  flex: 0 0 auto;
  display: flex;
  justify-content: center;
  padding: 10px 24px;
  border-bottom: 1px solid transparent;
}

.editor-meta {
  font-size: 12px;
  color: var(--text-secondary);
}

.editor-canvas {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 0 48px 48px;
  max-width: 760px;
  margin: 0 auto;
  width: 100%;
}

.note-title {
  font-size: 26px;
  font-weight: 700;
  margin: 4px 0 16px;
  color: var(--text-primary);
}

.note-line {
  font-size: 15px;
  line-height: 1.6;
  color: var(--text-primary);
  margin: 0 0 4px;
  white-space: pre-wrap;
}

.empty-editor {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--text-tertiary);
}

.empty-editor svg {
  width: 48px;
  height: 48px;
}

.empty-editor p {
  font-size: 14px;
  margin: 0;
}
</style>
