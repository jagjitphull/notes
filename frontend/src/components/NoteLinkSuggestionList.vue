<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  items: string[];
  command: (title: string) => void;
}>();

const { t } = useI18n();
const selectedIndex = ref(0);

watch(
  () => props.items,
  () => {
    selectedIndex.value = 0;
  },
);

function select(index: number) {
  const title = props.items[index];
  if (title) props.command(title);
}

// Called by the NoteLink extension's Suggestion onKeyDown - see noteLink.ts.
function onKeyDown({ event }: { event: KeyboardEvent }): boolean {
  if (props.items.length === 0) return false;
  if (event.key === "ArrowDown") {
    selectedIndex.value = (selectedIndex.value + 1) % props.items.length;
    return true;
  }
  if (event.key === "ArrowUp") {
    selectedIndex.value = (selectedIndex.value - 1 + props.items.length) % props.items.length;
    return true;
  }
  if (event.key === "Enter") {
    select(selectedIndex.value);
    return true;
  }
  return false;
}

defineExpose({ onKeyDown });
</script>

<template>
  <div class="note-link-suggestions" role="listbox">
    <button
      v-for="(title, index) in items"
      :key="title"
      type="button"
      role="option"
      class="note-link-suggestion-item"
      :class="{ active: index === selectedIndex }"
      :aria-selected="index === selectedIndex"
      @click="select(index)"
      @mouseenter="selectedIndex = index"
    >
      {{ title }}
    </button>
    <p v-if="items.length === 0" class="note-link-suggestions-empty">
      {{ t('editor.noteLink.noMatches') }}
    </p>
  </div>
</template>

<style scoped>
.note-link-suggestions {
  min-width: 180px;
  max-width: 280px;
  max-height: 240px;
  overflow-y: auto;
  background: var(--bg-list);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  padding: 4px;
}

.note-link-suggestion-item {
  display: block;
  width: 100%;
  text-align: left;
  padding: 6px 10px;
  border: none;
  background: transparent;
  border-radius: 6px;
  font-size: 13px;
  color: var(--text-primary);
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.note-link-suggestion-item.active {
  background: var(--bg-hover);
}

.note-link-suggestions-empty {
  margin: 0;
  padding: 6px 10px;
  font-size: 12.5px;
  color: var(--text-tertiary);
}
</style>
