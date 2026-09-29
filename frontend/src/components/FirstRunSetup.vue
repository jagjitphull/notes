<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { documentDir, join } from "@tauri-apps/api/path";
import { detectCloudFolders, setNotesRoot, type CloudFolderSuggestion } from "../api";

const emit = defineEmits<{ ready: [] }>();

const suggestions = ref<CloudFolderSuggestion[]>([]);
const busy = ref(false);
const error = ref<string | null>(null);

onMounted(async () => {
  suggestions.value = await detectCloudFolders();
});

async function chooseRoot(path: string) {
  busy.value = true;
  error.value = null;
  try {
    await setNotesRoot(path);
    emit("ready");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function pickFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") {
    await chooseRoot(selected);
  }
}

async function useDefaultLocal() {
  const path = await join(await documentDir(), "Notes");
  await chooseRoot(path);
}
</script>

<template>
  <div class="setup-screen">
    <div class="setup-card">
      <h1>Where should your notes live?</h1>
      <p class="subtitle">
        Notes are saved as plain Markdown files in a folder you choose. Pick a
        folder inside Dropbox, pCloud, or any synced drive to have your notes
        follow you across Mac and Linux — or keep it local.
      </p>

      <div v-if="suggestions.length" class="suggestions">
        <button
          v-for="s in suggestions"
          :key="s.path"
          class="suggestion"
          :disabled="busy"
          @click="chooseRoot(`${s.path}/Notes`)"
        >
          <span class="suggestion-name">{{ s.name }}</span>
          <span class="suggestion-path">{{ s.path }}/Notes</span>
        </button>
      </div>

      <div class="actions">
        <button class="primary" :disabled="busy" @click="pickFolder">
          Choose a Folder…
        </button>
        <button class="secondary" :disabled="busy" @click="useDefaultLocal">
          Use Documents/Notes (local only)
        </button>
      </div>

      <p v-if="error" class="error" role="alert">{{ error }}</p>
    </div>
  </div>
</template>

<style scoped>
.setup-screen {
  height: 100vh;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-editor);
}

.setup-card {
  width: 100%;
  max-width: 440px;
  padding: 32px;
  text-align: center;
}

h1 {
  font-size: 22px;
  margin: 0 0 12px;
  color: var(--text-primary);
}

.subtitle {
  font-size: 13.5px;
  line-height: 1.5;
  color: var(--text-secondary);
  margin: 0 0 24px;
}

.suggestions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 20px;
}

.suggestion {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-list);
  cursor: pointer;
  text-align: left;
}

.suggestion:hover {
  background: var(--bg-hover);
}

.suggestion-name {
  font-weight: 600;
  font-size: 13.5px;
  color: var(--text-primary);
}

.suggestion-path {
  font-size: 12px;
  color: var(--text-tertiary);
}

.actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

button.primary,
button.secondary {
  padding: 9px 16px;
  border-radius: 8px;
  border: none;
  font-size: 13.5px;
  font-weight: 600;
  cursor: pointer;
}

button.primary {
  background: var(--bg-selected-strong);
  color: #fff;
}

button.secondary {
  background: var(--bg-hover);
  color: var(--text-primary);
}

button:disabled {
  opacity: 0.6;
  cursor: default;
}

.error {
  margin-top: 16px;
  font-size: 12.5px;
  color: var(--danger);
}
</style>
