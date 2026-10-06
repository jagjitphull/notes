<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Icon from "./icons/Icon.vue";
import type { SlashCommandItem } from "../tiptap/slashCommand";

const props = defineProps<{
  items: SlashCommandItem[];
  command: (item: SlashCommandItem) => void;
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
  const item = props.items[index];
  if (item) props.command(item);
}

// Called by the SlashCommand extension's Suggestion onKeyDown - see
// tiptap/slashCommand.ts.
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
  <div class="slash-command-list" role="listbox">
    <button
      v-for="(item, index) in items"
      :key="item.id"
      type="button"
      role="option"
      class="slash-command-item"
      :class="{ active: index === selectedIndex }"
      :aria-selected="index === selectedIndex"
      @click="select(index)"
      @mouseenter="selectedIndex = index"
    >
      <Icon class="slash-command-icon" :name="item.icon" />
      <span>{{ item.label }}</span>
    </button>
    <p v-if="items.length === 0" class="slash-command-empty">
      {{ t('editor.slashCommand.noMatches') }}
    </p>
  </div>
</template>

<style scoped>
.slash-command-list {
  min-width: 200px;
  max-width: 280px;
  max-height: 320px;
  overflow-y: auto;
  background: var(--bg-list);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  padding: 4px;
}

.slash-command-item {
  display: flex;
  align-items: center;
  gap: 8px;
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

.slash-command-item.active {
  background: var(--bg-hover);
}

.slash-command-icon {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  color: var(--text-secondary);
}

.slash-command-empty {
  margin: 0;
  padding: 6px 10px;
  font-size: 12.5px;
  color: var(--text-tertiary);
}
</style>
