<script setup lang="ts">
import { ref } from "vue";
import type { Folder, Tag } from "../types";
import { useTheme, type ThemePreference } from "../composables/useTheme";

defineProps<{
  folders: Folder[];
  tags: Tag[];
  allCount: number;
  deletedCount: number;
  folderCounts: Record<string, number>;
}>();

const emit = defineEmits<{
  newFolder: [];
  folderContextmenu: [event: MouseEvent, folder: Folder];
}>();

const selectedId = defineModel<string>("selectedId", { required: true });
const searchQuery = defineModel<string>("searchQuery", { required: true });

const { preference, cyclePreference } = useTheme();

const themeLabels: Record<ThemePreference, string> = {
  system: "System",
  light: "Light",
  dark: "Dark",
};

const searchInputRef = ref<HTMLInputElement | null>(null);

function focusSearch() {
  searchInputRef.value?.focus();
  searchInputRef.value?.select();
}

defineExpose({ focusSearch });
</script>

<template>
  <nav class="sidebar">
    <div class="search-box">
      <svg viewBox="0 0 20 20" fill="none" aria-hidden="true">
        <circle cx="8.5" cy="8.5" r="6" stroke="currentColor" stroke-width="1.6" />
        <path d="M13 13L17.5 17.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
      </svg>
      <input
        ref="searchInputRef"
        v-model="searchQuery"
        type="text"
        placeholder="Search"
        aria-label="Search notes"
      />
    </div>

    <ul class="nav-list">
      <li>
        <button
          class="nav-item"
          :class="{ active: selectedId === 'all' }"
          @click="selectedId = 'all'"
        >
          <svg class="nav-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <rect x="3" y="3" width="14" height="14" rx="3" stroke="currentColor" stroke-width="1.5" />
            <path d="M6.5 7.5h7M6.5 10h7M6.5 12.5h4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
          <span class="nav-label">All Notes</span>
          <span class="nav-count">{{ allCount }}</span>
        </button>
      </li>
    </ul>

    <div class="section">
      <div class="section-header">
        <h2 class="pane-title">Folders</h2>
        <button
          class="icon-button"
          title="New Folder"
          aria-label="New Folder"
          @click="emit('newFolder')"
        >
          <svg viewBox="0 0 20 20" fill="none" aria-hidden="true">
            <path d="M10 4v12M4 10h12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
          </svg>
        </button>
      </div>
      <ul class="nav-list">
        <li v-for="folder in folders" :key="folder.id">
          <button
            class="nav-item"
            :class="{ active: selectedId === folder.id }"
            @click="selectedId = folder.id"
            @contextmenu.prevent="emit('folderContextmenu', $event, folder)"
          >
            <svg class="nav-icon folder-icon" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
              <path
                d="M3 5.5A1.5 1.5 0 0 1 4.5 4h3.379a1.5 1.5 0 0 1 1.06.44l1.122 1.12a1.5 1.5 0 0 0 1.06.44H15.5A1.5 1.5 0 0 1 17 7.5v7A1.5 1.5 0 0 1 15.5 16h-11A1.5 1.5 0 0 1 3 14.5v-9Z"
              />
            </svg>
            <span class="nav-label">{{ folder.name }}</span>
            <span class="nav-count">{{ folderCounts[folder.id] ?? 0 }}</span>
          </button>
        </li>
      </ul>
    </div>

    <div class="section">
      <div class="section-header">
        <h2 class="pane-title">Tags</h2>
      </div>
      <ul class="nav-list">
        <li v-for="tag in tags" :key="tag.id">
          <button class="nav-item" :class="{ active: selectedId === `tag:${tag.id}` }" @click="selectedId = `tag:${tag.id}`">
            <span class="nav-icon tag-icon" aria-hidden="true">#</span>
            <span class="nav-label">{{ tag.name }}</span>
          </button>
        </li>
      </ul>
    </div>

    <div class="sidebar-footer">
      <button
        class="nav-item"
        :class="{ active: selectedId === 'recently-deleted' }"
        @click="selectedId = 'recently-deleted'"
      >
        <svg class="nav-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <path
            d="M5 6.5h10M8.25 6.5V5a1 1 0 0 1 1-1h1.5a1 1 0 0 1 1 1v1.5M8.5 9.5v4M11.5 9.5v4M5.75 6.5l.6 8.1a1.5 1.5 0 0 0 1.496 1.4h4.308a1.5 1.5 0 0 0 1.496-1.4l.6-8.1"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <span class="nav-label">Recently Deleted</span>
        <span class="nav-count">{{ deletedCount }}</span>
      </button>

      <button class="theme-toggle" @click="cyclePreference" :title="`Appearance: ${themeLabels[preference]} (click to change)`">
        <svg v-if="preference === 'dark'" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
          <path d="M10 2.5a7.5 7.5 0 1 0 7.35 9.02.75.75 0 0 0-.9-.88 5.8 5.8 0 0 1-7.09-7.09.75.75 0 0 0-.88-.9c-.36.06-.72.13-1.06.23A7.53 7.53 0 0 0 10 2.5Z" />
        </svg>
        <svg v-else-if="preference === 'light'" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
          <circle cx="10" cy="10" r="3.5" />
          <path d="M10 2v2M10 16v2M18 10h-2M4 10H2M15.36 4.64l-1.42 1.42M6.06 13.94l-1.42 1.42M15.36 15.36l-1.42-1.42M6.06 6.06 4.64 4.64" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
        </svg>
        <svg v-else viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <rect x="2.5" y="4.5" width="15" height="10" rx="1.5" stroke="currentColor" stroke-width="1.4" />
          <path d="M7 17.5h6" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
        </svg>
        <span class="nav-label">{{ themeLabels[preference] }}</span>
      </button>
    </div>
  </nav>
</template>

<style scoped>
.sidebar {
  height: 100%;
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
