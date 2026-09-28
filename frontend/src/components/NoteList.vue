<script setup lang="ts">
import { computed } from "vue";
import type { Note } from "../types";

const props = defineProps<{
  notes: Note[];
  title: string;
  showPinnedSections: boolean;
  canCreate: boolean;
}>();

const emit = defineEmits<{
  create: [];
  contextmenu: [event: MouseEvent, note: Note];
}>();

const selectedId = defineModel<string | null>("selectedId", { default: null });

const pinnedCount = computed(() =>
  props.showPinnedSections ? props.notes.filter((n) => n.isPinned).length : 0,
);

function sectionFor(index: number): "pinned" | "notes" | null {
  if (!props.showPinnedSections || pinnedCount.value === 0) return null;
  if (index === 0) return "pinned";
  if (index === pinnedCount.value) return "notes";
  return null;
}

function formatDate(iso: string): string {
  const date = new Date(iso);
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  if (sameDay) {
    return date.toLocaleTimeString(undefined, {
      hour: "numeric",
      minute: "2-digit",
    });
  }
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) return "Yesterday";

  const daysAgo = (now.getTime() - date.getTime()) / 86_400_000;
  if (daysAgo < 6) {
    return date.toLocaleDateString(undefined, { weekday: "long" });
  }
  return date.toLocaleDateString(undefined, {
    month: "numeric",
    day: "numeric",
    year: "2-digit",
  });
}

function preview(note: Note): string {
  return note.plaintextContent.split("\n").slice(0, 2).join(" ");
}
</script>

<template>
  <section class="note-list">
    <div class="pane-header">
      <h1 class="list-title">{{ title }}</h1>
      <button
        v-if="canCreate"
        class="icon-button new-note-button"
        title="New Note"
        aria-label="New Note"
        @click="emit('create')"
      >
        <svg viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <path d="M10 4v12M4 10h12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
        </svg>
      </button>
    </div>

    <div class="scroll-area">
      <p v-if="notes.length === 0" class="empty-state">No Notes</p>

      <ul v-else class="items">
        <template v-for="(note, index) in notes" :key="note.id">
          <li v-if="sectionFor(index) === 'pinned'" class="section-label">Pinned</li>
          <li v-if="sectionFor(index) === 'notes'" class="section-label">Notes</li>
          <li>
            <button
              class="note-item"
              :class="{ active: selectedId === note.id }"
              @click="selectedId = note.id"
              @contextmenu.prevent="
                selectedId = note.id;
                emit('contextmenu', $event, note);
              "
            >
              <div class="note-title-row">
                <span class="note-title">{{ note.title || "New Note" }}</span>
                <svg v-if="note.isPinned" class="pin-icon" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                  <path d="M11.5 2.5a1 1 0 0 1 1.4 0l4.6 4.6a1 1 0 0 1 0 1.4l-.7.7a1 1 0 0 1-1.4 0l-.2-.2-2.6 2.6.6 2.9a.75.75 0 0 1-1.27.68l-2.9-2.9-4 4a.6.6 0 0 1-.85-.85l4-4-2.9-2.9a.75.75 0 0 1 .68-1.27l2.9.6 2.6-2.6-.2-.2a1 1 0 0 1 0-1.4l.7-.7Z" />
                </svg>
              </div>
              <div class="note-meta">{{ formatDate(note.updatedAt) }}</div>
              <div class="note-preview">{{ preview(note) }}</div>
            </button>
          </li>
        </template>
      </ul>
    </div>
  </section>
</template>

<style scoped>
.note-list {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-list);
  border-right: 1px solid var(--border);
  overflow: hidden;
}

.pane-header {
  justify-content: space-between;
}

.list-title {
  font-size: 20px;
  font-weight: 700;
  margin: 0;
  color: var(--text-primary);
}

.new-note-button {
  flex: 0 0 auto;
}

.scroll-area {
  flex: 1 1 auto;
  overflow-y: auto;
  padding: 0 6px 12px;
}

.empty-state {
  text-align: center;
  color: var(--text-tertiary);
  margin-top: 40px;
  font-size: 13px;
}

.items {
  list-style: none;
  margin: 0;
  padding: 0;
}

.section-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
  padding: 10px 8px 4px;
}

.note-item {
  display: block;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  border-radius: 8px;
  padding: 8px 10px;
  cursor: pointer;
  border-bottom: 1px solid var(--border);
}

.note-item:hover {
  background: var(--bg-hover);
}

.note-item.active {
  background: var(--bg-selected-strong);
}

.note-item.active .note-title,
.note-item.active .note-meta,
.note-item.active .note-preview {
  color: #fff;
}

.note-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.note-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1 1 auto;
}

.pin-icon {
  width: 12px;
  height: 12px;
  flex: 0 0 auto;
  color: var(--text-tertiary);
}

.note-item.active .pin-icon {
  color: rgba(255, 255, 255, 0.75);
}

.note-meta {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 2px;
}

.note-preview {
  font-size: 12.5px;
  color: var(--text-secondary);
  margin-top: 2px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
