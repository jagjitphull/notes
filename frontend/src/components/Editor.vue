<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { Folder, Note } from "../types";

const props = defineProps<{
  note: Note | null;
  folders: Folder[];
}>();

const emit = defineEmits<{
  togglePin: [];
  toggleDeleted: [];
}>();

const body = defineModel<string>("body", { default: "" });

const textareaRef = ref<HTMLTextAreaElement | null>(null);

watch(
  () => props.note?.id,
  async () => {
    await nextTick();
    textareaRef.value?.focus();
  },
);

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

const isDeleted = computed(() => !!props.note?.deletedAt);

function onInput(e: Event) {
  body.value = (e.target as HTMLTextAreaElement).value;
}
</script>

<template>
  <main class="editor">
    <template v-if="note">
      <div class="editor-toolbar">
        <span class="editor-meta">{{ formattedDate }} &middot; {{ folderName }}</span>
        <div class="editor-actions">
          <button
            class="icon-button"
            :class="{ active: note.isPinned }"
            :title="note.isPinned ? 'Unpin' : 'Pin'"
            :disabled="isDeleted"
            @click="emit('togglePin')"
          >
            <svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
              <path d="M11.5 2.5a1 1 0 0 1 1.4 0l4.6 4.6a1 1 0 0 1 0 1.4l-.7.7a1 1 0 0 1-1.4 0l-.2-.2-2.6 2.6.6 2.9a.75.75 0 0 1-1.27.68l-2.9-2.9-4 4a.6.6 0 0 1-.85-.85l4-4-2.9-2.9a.75.75 0 0 1 .68-1.27l2.9.6 2.6-2.6-.2-.2a1 1 0 0 1 0-1.4l.7-.7Z" />
            </svg>
          </button>
          <button
            class="icon-button"
            :title="isDeleted ? 'Restore' : 'Delete'"
            @click="emit('toggleDeleted')"
          >
            <svg v-if="isDeleted" viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <path d="M4 10a6 6 0 1 1 2 4.5M4 10V6M4 10h4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <svg v-else viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <path
                d="M5 6.5h10M8.25 6.5V5a1 1 0 0 1 1-1h1.5a1 1 0 0 1 1 1v1.5M8.5 9.5v4M11.5 9.5v4M5.75 6.5l.6 8.1a1.5 1.5 0 0 0 1.496 1.4h4.308a1.5 1.5 0 0 0 1.496-1.4l.6-8.1"
                stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
              />
            </svg>
          </button>
        </div>
      </div>
      <div class="editor-canvas">
        <textarea
          ref="textareaRef"
          class="editor-textarea"
          :value="body"
          :readonly="isDeleted"
          placeholder="New Note"
          @input="onInput"
        ></textarea>
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
  align-items: center;
  justify-content: center;
  position: relative;
  padding: 10px 16px;
}

.editor-meta {
  font-size: 12px;
  color: var(--text-secondary);
}

.editor-actions {
  position: absolute;
  right: 12px;
  top: 6px;
  display: flex;
  gap: 2px;
}

.icon-button.active {
  color: var(--accent-blue);
}

.editor-canvas {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 0 48px 48px;
  max-width: 760px;
  margin: 0 auto;
  width: 100%;
  display: flex;
}

.editor-textarea {
  flex: 1 1 auto;
  border: none;
  outline: none;
  resize: none;
  background: transparent;
  color: var(--text-primary);
  font: inherit;
  font-size: 15px;
  line-height: 1.6;
  font-family: var(--sans);
  padding: 4px 0 0;
}

.editor-textarea::placeholder {
  color: var(--text-tertiary);
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
