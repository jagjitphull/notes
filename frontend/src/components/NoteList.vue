<script setup lang="ts">
import { computed, ref, watch, type ComponentPublicInstance } from "vue";
import { useI18n } from "vue-i18n";
import { useVirtualizer } from "@tanstack/vue-virtual";
import Icon from "./icons/Icon.vue";
import type { Note } from "../types";

const props = defineProps<{
  notes: Note[];
  title: string;
  showPinnedSections: boolean;
  canCreate: boolean;
  hasTemplates: boolean;
  isTrash: boolean;
  isTemplates: boolean;
}>();

const emit = defineEmits<{
  create: [];
  createContextmenu: [event: MouseEvent];
  contextmenu: [event: MouseEvent, note: Note];
  sortClick: [event: MouseEvent];
}>();

const selectedId = defineModel<string | null>("selectedId", { default: null });

const { t } = useI18n();

// Flattened once here (section labels + note rows interleaved), rather
// than recomputed per-row from an index during render (the pre-virtualized
// version's sectionFor(index)) - the virtualizer needs a single flat,
// index-addressable row list up front to size and position things, and
// pulling this out keeps that list the one place the two row kinds mix.
type Row = { kind: "section"; key: string; label: string } | { kind: "note"; key: string; note: Note };

const rows = computed<Row[]>(() => {
  const pinnedCount = props.showPinnedSections ? props.notes.filter((n) => n.isPinned).length : 0;
  const out: Row[] = [];
  props.notes.forEach((note, index) => {
    if (pinnedCount > 0 && index === 0) {
      out.push({ kind: "section", key: "section-pinned", label: t("noteList.pinned") });
    }
    if (pinnedCount > 0 && index === pinnedCount) {
      out.push({ kind: "section", key: "section-notes", label: t("common.notes") });
    }
    out.push({ kind: "note", key: note.id, note });
  });
  return out;
});

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

const scrollAreaRef = ref<HTMLElement | null>(null);

// Estimates only - refined against each row's real rendered height once it
// first mounts (see :ref="measureRow" below), so these don't need to be
// pixel-perfect across platforms/font rendering, just close enough that
// the initial scrollbar size/position isn't visibly wrong before that
// measurement pass catches up.
const SECTION_ESTIMATE = 36;
const NOTE_ESTIMATE = 92;

const virtualizer = useVirtualizer(
  computed(() => ({
    count: rows.value.length,
    getScrollElement: () => scrollAreaRef.value,
    estimateSize: (index: number) => (rows.value[index]?.kind === "section" ? SECTION_ESTIMATE : NOTE_ESTIMATE),
    getItemKey: (index: number) => rows.value[index]?.key ?? index,
    overscan: 8,
  })),
);

function measureRow(el: Element | ComponentPublicInstance | null) {
  if (el instanceof HTMLElement) virtualizer.value.measureElement(el);
}

// Pairs each rendered virtual row with its actual data, so the template
// below can switch on item.row.kind once per row instead of repeatedly
// indexing into rows[] (and re-asserting its type) at every usage site.
const visibleRows = computed(() =>
  virtualizer.value.getVirtualItems().map((v) => ({ index: v.index, start: v.start, row: rows.value[v.index] })),
);

// Keyboard arrow-key navigation (App.vue's navigateList) just moves
// selectedId by index - it doesn't know or care which rows are actually
// rendered, so without this, moving selection past the virtualized
// window wouldn't visibly do anything until the user scrolled there
// themselves.
watch(selectedId, (id) => {
  if (id === null) return;
  const index = rows.value.findIndex((r) => r.kind === "note" && r.note.id === id);
  if (index !== -1) virtualizer.value.scrollToIndex(index, { align: "auto" });
});

defineExpose({ virtualizer });
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
      <div v-if="canCreate" class="new-note-group">
        <button
          class="icon-button new-note-button"
          :title="t('common.newNote')"
          :aria-label="t('common.newNote')"
          @click="emit('create')"
          @contextmenu.prevent="emit('createContextmenu', $event)"
        >
          <Icon name="plus" />
        </button>
        <button
          v-if="hasTemplates"
          class="icon-button new-note-caret"
          :title="t('noteList.newFromTemplate')"
          :aria-label="t('noteList.newFromTemplate')"
          @click="emit('createContextmenu', $event)"
        >
          <Icon name="chevronDown" />
        </button>
      </div>
    </div>

    <p v-if="isTrash" class="trash-notice">
      {{ t('noteList.trashNotice') }}
    </p>

    <div ref="scrollAreaRef" class="scroll-area">
      <p v-if="notes.length === 0 && isTemplates" class="empty-state">
        {{ t('noteList.emptyTemplates') }}
      </p>
      <p v-else-if="notes.length === 0" class="empty-state">{{ t('noteList.empty') }}</p>

      <div
        v-else
        role="list"
        class="items"
        :style="{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }"
      >
        <template v-for="item in visibleRows" :key="item.row.key">
          <div
            v-if="item.row.kind === 'section'"
            :ref="measureRow"
            :data-index="item.index"
            class="section-label"
            :style="{ transform: `translateY(${item.start}px)` }"
          >
            {{ item.row.label }}
          </div>
          <div
            v-else
            :ref="measureRow"
            :data-index="item.index"
            role="listitem"
            :style="{ transform: `translateY(${item.start}px)` }"
            class="row"
          >
            <button
              class="note-item"
              :class="{ active: selectedId === item.row.note.id }"
              :aria-current="selectedId === item.row.note.id ? 'true' : undefined"
              @click="selectedId = item.row.note.id"
              @contextmenu.prevent="
                selectedId = item.row.note.id;
                emit('contextmenu', $event, item.row.note);
              "
            >
              <div class="note-title-row">
                <span class="note-title">{{ item.row.note.title || t('common.newNote') }}</span>
                <Icon v-if="item.row.note.isPinned" class="pin-icon" name="pin" />
              </div>
              <div class="note-meta">
                {{ formatDate(item.row.note.updatedAt) }}
              </div>
              <div class="note-preview">{{ preview(item.row.note) }}</div>
            </button>
          </div>
        </template>
      </div>
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
.new-note-group {
  flex: 0 0 auto;
}

.new-note-group {
  display: flex;
  align-items: center;
  gap: 2px;
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

.row,
.section-label {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  box-sizing: border-box;
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
  box-sizing: border-box;
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
