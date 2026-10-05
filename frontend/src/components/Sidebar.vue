<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import Icon from "./icons/Icon.vue";
import type { Folder, Tag } from "../types";
import { useTheme, type ThemePreference } from "../composables/useTheme";

defineProps<{
  folders: Folder[];
  tags: Tag[];
  allCount: number;
  deletedCount: number;
  templateCount: number;
  folderCounts: Record<string, number>;
  smartSearchSupported: boolean;
}>();

const emit = defineEmits<{
  newFolder: [];
  openToday: [];
  importMarkdown: [];
  folderContextmenu: [event: MouseEvent, folder: Folder];
  tagContextmenu: [event: MouseEvent, tag: Tag];
}>();

const selectedId = defineModel<string>("selectedId", { required: true });
const searchQuery = defineModel<string>("searchQuery", { required: true });
const smartSearchEnabled = defineModel<boolean>("smartSearchEnabled", { required: true });

const { t } = useI18n();
const { preference, cyclePreference } = useTheme();

const themeLabels = computed<Record<ThemePreference, string>>(() => ({
  system: t("sidebar.theme.system"),
  light: t("sidebar.theme.light"),
  dark: t("sidebar.theme.dark"),
}));

const searchInputRef = ref<HTMLInputElement | null>(null);

function focusSearch() {
  searchInputRef.value?.focus();
  searchInputRef.value?.select();
}

defineExpose({ focusSearch });
</script>

<template>
  <nav class="sidebar" :aria-label="t('sidebar.navLabel')">
    <div class="search-box">
      <Icon name="search" />
      <input
        ref="searchInputRef"
        v-model="searchQuery"
        type="text"
        :placeholder="t('sidebar.searchPlaceholder')"
        :aria-label="t('sidebar.searchLabel')"
      />
      <button
        v-if="smartSearchSupported"
        class="smart-search-toggle"
        :class="{ active: smartSearchEnabled }"
        type="button"
        :title="t('sidebar.smartSearchTitle')"
        :aria-label="t('sidebar.smartSearchToggle')"
        :aria-pressed="smartSearchEnabled"
        @click="smartSearchEnabled = !smartSearchEnabled"
      >
        <Icon name="sparkle" />
      </button>
    </div>

    <ul class="nav-list">
      <li>
        <button
          class="nav-item"
          :class="{ active: selectedId === 'all' }"
          :aria-current="selectedId === 'all' ? 'true' : undefined"
          @click="selectedId = 'all'"
        >
          <Icon class="nav-icon" name="notesList" />
          <span class="nav-label">{{ t('common.allNotes') }}</span>
          <span class="nav-count">{{ allCount }}</span>
        </button>
      </li>
      <li>
        <button class="nav-item" @click="emit('openToday')">
          <Icon class="nav-icon" name="today" />
          <span class="nav-label">{{ t('common.today') }}</span>
        </button>
      </li>
    </ul>

    <div class="section">
      <div class="section-header">
        <h2 class="pane-title">{{ t('sidebar.folders') }}</h2>
        <button
          class="icon-button"
          :title="t('common.newFolder')"
          :aria-label="t('common.newFolder')"
          @click="emit('newFolder')"
        >
          <Icon name="plus" />
        </button>
      </div>
      <ul class="nav-list">
        <li v-for="folder in folders" :key="folder.id">
          <button
            class="nav-item"
            :class="{ active: selectedId === folder.id }"
            :aria-current="selectedId === folder.id ? 'true' : undefined"
            @click="selectedId = folder.id"
            @contextmenu.prevent="emit('folderContextmenu', $event, folder)"
          >
            <Icon
              class="nav-icon folder-icon"
              name="folder"
              :style="folder.color ? { color: folder.color } : undefined"
            />
            <span class="nav-label">{{ folder.name }}</span>
            <span class="nav-count">{{ folderCounts[folder.id] ?? 0 }}</span>
          </button>
        </li>
      </ul>
    </div>

    <div class="section">
      <div class="section-header">
        <h2 class="pane-title">{{ t('sidebar.tags') }}</h2>
      </div>
      <ul class="nav-list">
        <li v-for="tag in tags" :key="tag.id">
          <button
            class="nav-item"
            :class="{ active: selectedId === `tag:${tag.id}` }"
            :aria-current="selectedId === `tag:${tag.id}` ? 'true' : undefined"
            @click="selectedId = `tag:${tag.id}`"
            @contextmenu.prevent="emit('tagContextmenu', $event, tag)"
          >
            <span
              class="nav-icon tag-icon"
              aria-hidden="true"
              :style="tag.color ? { color: tag.color } : undefined"
              >#</span
            >
            <span class="nav-label">{{ tag.name }}</span>
          </button>
        </li>
      </ul>
    </div>

    <div class="sidebar-footer">
      <button
        class="nav-item"
        :class="{ active: selectedId === 'templates' }"
        :aria-current="selectedId === 'templates' ? 'true' : undefined"
        @click="selectedId = 'templates'"
      >
        <Icon class="nav-icon" name="template" />
        <span class="nav-label">{{ t('common.templates') }}</span>
        <span class="nav-count">{{ templateCount }}</span>
      </button>

      <button
        class="nav-item"
        :class="{ active: selectedId === 'graph' }"
        :aria-current="selectedId === 'graph' ? 'true' : undefined"
        @click="selectedId = 'graph'"
      >
        <Icon class="nav-icon" name="graph" />
        <span class="nav-label">{{ t('graph.navLabel') }}</span>
      </button>

      <button
        class="nav-item"
        :class="{ active: selectedId === 'recently-deleted' }"
        :aria-current="selectedId === 'recently-deleted' ? 'true' : undefined"
        @click="selectedId = 'recently-deleted'"
      >
        <Icon class="nav-icon" name="trash" />
        <span class="nav-label">{{ t('common.recentlyDeleted') }}</span>
        <span class="nav-count">{{ deletedCount }}</span>
      </button>

      <button class="nav-item" @click="emit('importMarkdown')">
        <Icon class="nav-icon" name="upload" />
        <span class="nav-label">{{ t('import.button') }}</span>
      </button>

      <button
        class="theme-toggle"
        @click="cyclePreference"
        :title="t('sidebar.appearanceTitle', { label: themeLabels[preference] })"
        :aria-label="t('sidebar.appearanceLabel', { label: themeLabels[preference] })"
      >
        <Icon :name="preference === 'dark' ? 'moon' : preference === 'light' ? 'sun' : 'monitor'" />
        <span class="nav-label">{{ themeLabels[preference] }}</span>
      </button>
    </div>
  </nav>
</template>

<style scoped>
.sidebar {
  height: 100%;
  /* A grid item (in App.vue's .app-shell) with its own overflow-y -
     needs min-height: 0 so a long folder/tag list actually scrolls
     within the pane instead of inflating the shared grid row's
     height. See NoteList.vue's .note-list comment for more. */
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  overflow-y: auto;
  padding: 12px 8px;
}

.search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  background: var(--bg-hover);
  border-radius: 8px;
  padding: 6px 8px;
  margin: 0 4px 12px;
  color: var(--text-secondary);
}

.search-box svg {
  width: 15px;
  height: 15px;
  flex: 0 0 auto;
}

.search-box input {
  border: none;
  background: transparent;
  outline: none;
  color: var(--text-primary);
  font: inherit;
  width: 100%;
}

.search-box input::placeholder {
  color: var(--text-tertiary);
}

.smart-search-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 0 0 auto;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--text-tertiary);
  cursor: pointer;
}

.smart-search-toggle svg {
  width: 14px;
  height: 14px;
}

.smart-search-toggle:hover {
  color: var(--text-secondary);
  background: rgba(127, 127, 127, 0.15);
}

.smart-search-toggle.active {
  color: var(--accent-blue);
  background: rgba(10, 132, 255, 0.15);
}

.nav-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  border: none;
  background: transparent;
  border-radius: 7px;
  padding: 6px 8px;
  margin: 1px 0;
  font-size: 13.5px;
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
}

.nav-item:hover {
  background: var(--bg-hover);
}

.nav-item.active {
  background: var(--bg-selected-strong);
  color: #fff;
}

.nav-item.active .nav-count {
  color: rgba(255, 255, 255, 0.75);
}

.nav-icon {
  width: 17px;
  height: 17px;
  flex: 0 0 auto;
  color: var(--accent-blue);
}

.folder-icon {
  color: var(--accent-blue);
}

.tag-icon {
  width: 17px;
  text-align: center;
  color: var(--text-secondary);
  font-weight: 600;
}

.nav-label {
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nav-count {
  font-size: 12px;
  color: var(--text-tertiary);
}

.section {
  margin-top: 14px;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px 2px;
}

.section-header .pane-title {
  padding: 0;
}

.sidebar-footer {
  margin-top: auto;
  padding-top: 10px;
  border-top: 1px solid var(--border);
}

.theme-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  border: none;
  background: transparent;
  border-radius: 7px;
  padding: 6px 8px;
  margin-top: 2px;
  font-size: 13.5px;
  color: var(--text-secondary);
  cursor: pointer;
}

.theme-toggle:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.theme-toggle svg {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
}
</style>
