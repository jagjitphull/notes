<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import Icon from "./icons/Icon.vue";
import type { Note } from "../types";

const props = defineProps<{
  notes: Note[];
  title: string;
  showPinnedSections: boolean;
  canCreate: boolean;
  isTrash: boolean;
}>();

const emit = defineEmits<{
  create: [];
  contextmenu: [event: MouseEvent, note: Note];
  sortClick: [event: MouseEvent];
}>();

const selectedId = defineModel<string | null>("selectedId", { default: null });

const { t } = useI18n();

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
  <section class="note-list" :aria-label="t('noteList.sectionLabel')">
    <div class="pane-header">
      <h2 class="list-title">{{ title }}</h2>
      <button
        class="icon-button sort-button"
        :title="t('noteList.sort.label')"
        :aria-label="t('noteList.sort.label')"
        @click="emit('sortClick', $event)"
      >
        <Icon name="sort" />
      </button>
      <button
        v-if="canCreate"
        class="icon-button new-note-button"
        :title="t('common.newNote')"
        :aria-label="t('common.newNote')"
        @click="emit('create')"
      >
        <Icon name="plus" />
      </button>
    </div>

    <p v-if="isTrash" class="trash-notice">
      {{ t('noteList.trashNotice') }}
    </p>

    <div class="scroll-area">
      <p v-if="notes.length === 0" class="empty-state">{{ t('noteList.empty') }}</p>

      <ul v-else class="items">
        <template v-for="(note, index) in notes" :key="note.id">
          <li v-if="sectionFor(index) === 'pinned'" class="section-label">{{ t('noteList.pinned') }}</li>
          <li v-if="sectionFor(index) === 'notes'" class="section-label">{{ t('common.notes') }}</li>
          <li>
            <button
              class="note-item"
              :class="{ active: selectedId === note.id }"
              :aria-current="selectedId === note.id ? 'true' : undefined"
              @click="selectedId = note.id"
              @contextmenu.prevent="
                selectedId = note.id;
                emit('contextmenu', $event, note);
              "
            >
              <div class="note-title-row">
                <span class="note-title">{{ note.title || t('common.newNote') }}</span>
                <Icon v-if="note.isPinned" class="pin-icon" name="pin" />
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
  /* Also a grid item (in App.vue's .app-shell), which has the same
     content-based min-height default as flex - without this, a long
     note in the adjacent Editor pane could inflate the shared grid
     row's height and stretch this pane past the viewport too. */
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-list);
  border-right: 1px solid var(--border);
  overflow: hidden;
}

.list-title {
  flex: 1 1 auto;
  font-size: 20px;
  font-weight: 700;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-primary);
}

.sort-button,
.new-note-button {
  flex: 0 0 auto;
}

.trash-notice {
  font-size: 11.5px;
  color: var(--text-tertiary);
  text-align: center;
  margin: 0 12px 8px;
}

.scroll-area {
  flex: 1 1 auto;
  /* Flex items default to min-height: auto (content-based), which
     overrides flex-shrink and defeats overflow-y: auto - a long note
     list would just grow the box instead of scrolling within it. */
  min-height: 0;
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
