<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Icon from "./icons/Icon.vue";
import type { IconName } from "./icons/icons";
import type { Folder, Note, Tag } from "../types";

export interface PaletteAction {
  id: string;
  label: string;
  icon: IconName;
  run: () => void;
}

const props = defineProps<{
  notes: Note[];
  folders: Folder[];
  tags: Tag[];
  actions: PaletteAction[];
}>();

const emit = defineEmits<{
  close: [];
  selectNote: [id: string];
  selectFolder: [id: string];
  selectTag: [id: string];
}>();

const { t } = useI18n();

const query = ref("");
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLUListElement | null>(null);
let previouslyFocused: HTMLElement | null = null;

type ResultKind = "action" | "note" | "folder" | "tag";

interface PaletteResult {
  kind: ResultKind;
  key: string;
  label: string;
  icon: IconName;
  run: () => void;
}

const RESULT_LIMIT = 20;

const results = computed<PaletteResult[]>(() => {
  const q = query.value.trim().toLowerCase();

  const matchedActions = props.actions
    .filter((a) => !q || a.label.toLowerCase().includes(q))
    .map((a): PaletteResult => ({ kind: "action", key: `action:${a.id}`, label: a.label, icon: a.icon, run: a.run }));

  // Actions alone (recent/suggested-style) when there's nothing typed yet
  // - matching every note/folder/tag against an empty query would just
  // dump the entire workspace into the list.
  if (!q) return matchedActions;

  const matchedNotes = props.notes
    .filter((n) => !n.deletedAt && !n.isTemplate && n.title.toLowerCase().includes(q))
    .slice(0, RESULT_LIMIT)
    .map(
      (n): PaletteResult => ({
        kind: "note",
        key: `note:${n.id}`,
        label: n.title || t("common.newNote"),
        icon: "document",
        run: () => emit("selectNote", n.id),
      }),
    );

  const matchedFolders = props.folders
    .filter((f) => f.name.toLowerCase().includes(q))
    .map(
      (f): PaletteResult => ({
        kind: "folder",
        key: `folder:${f.id}`,
        label: f.name,
        icon: "folder",
        run: () => emit("selectFolder", f.id),
      }),
    );

  const matchedTags = props.tags
    .filter((tag) => tag.name.toLowerCase().includes(q))
    .map(
      (tag): PaletteResult => ({
        kind: "tag",
        key: `tag:${tag.id}`,
        label: tag.name,
        icon: "document",
        run: () => emit("selectTag", tag.id),
      }),
    );

  return [...matchedActions, ...matchedNotes, ...matchedFolders, ...matchedTags];
});

function sectionLabel(index: number): string | null {
  const kind = results.value[index]?.kind;
  if (!kind) return null;
  if (index > 0 && results.value[index - 1]?.kind === kind) return null;
  return t(`commandPalette.section.${kind}`);
}

function runActive() {
  const result = results.value[activeIndex.value];
  if (!result) return;
  result.run();
  emit("close");
}

function move(delta: number) {
  if (results.value.length === 0) return;
  activeIndex.value = ((activeIndex.value + delta) % results.value.length + results.value.length) % results.value.length;
  nextTick(() => {
    listRef.value?.children[activeIndex.value]?.scrollIntoView({ block: "nearest" });
  });
}

onMounted(async () => {
  previouslyFocused = document.activeElement as HTMLElement | null;
  await nextTick();
  inputRef.value?.focus();
});

onBeforeUnmount(() => {
  previouslyFocused?.focus();
});
</script>

<template>
  <Teleport to="body">
    <div class="palette-backdrop" @click.self="emit('close')">
      <div class="palette" role="dialog" aria-modal="true" :aria-label="t('commandPalette.title')">
        <div class="palette-input-row">
          <Icon name="search" class="palette-search-icon" />
          <input
            ref="inputRef"
            v-model="query"
            type="text"
            class="palette-input"
            :placeholder="t('commandPalette.placeholder')"
            @keydown.esc="emit('close')"
            @keydown.down.prevent="move(1)"
            @keydown.up.prevent="move(-1)"
            @keydown.enter.prevent="runActive"
            @input="activeIndex = 0"
          />
        </div>
        <p v-if="results.length === 0" class="palette-empty">{{ t('commandPalette.noResults') }}</p>
        <ul v-else ref="listRef" class="palette-results" role="listbox">
          <template v-for="(result, index) in results" :key="result.key">
            <li v-if="sectionLabel(index)" class="palette-section" role="presentation">
              {{ sectionLabel(index) }}
            </li>
            <li role="option" :aria-selected="index === activeIndex">
              <button
                class="palette-item"
                :class="{ active: index === activeIndex }"
                @mouseenter="activeIndex = index"
                @click="
                  result.run();
                  emit('close');
                "
              >
                <Icon :name="result.icon" class="palette-item-icon" />
                <span class="palette-item-label">{{ result.label }}</span>
              </button>
            </li>
          </template>
        </ul>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.palette-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1100;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 12vh;
}

.palette {
  width: 480px;
  max-width: calc(100vw - 32px);
  max-height: 60vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-list);
  border-radius: 10px;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.35);
  overflow: hidden;
}

.palette-input-row {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border);
}

.palette-search-icon {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  color: var(--text-tertiary);
}

.palette-input {
  flex: 1 1 auto;
  border: none;
  background: transparent;
  outline: none;
  color: var(--text-primary);
  font: inherit;
  font-size: 15px;
}

.palette-input::placeholder {
  color: var(--text-tertiary);
}

.palette-empty {
  margin: 0;
  padding: 24px 12px;
  text-align: center;
  font-size: 13px;
  color: var(--text-tertiary);
}

.palette-results {
  flex: 1 1 auto;
  overflow-y: auto;
  list-style: none;
  margin: 0;
  padding: 6px;
}

.palette-section {
  padding: 8px 10px 4px;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.02em;
  color: var(--text-tertiary);
}

.palette-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  border-radius: 7px;
  padding: 8px 10px;
  font-size: 13.5px;
  color: var(--text-primary);
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.palette-item.active {
  background: var(--bg-selected-strong);
  color: #fff;
}

.palette-item-icon {
  width: 15px;
  height: 15px;
  flex: 0 0 auto;
}
</style>
