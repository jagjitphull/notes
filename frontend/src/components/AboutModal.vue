<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getVersion } from "@tauri-apps/api/app";
import { openExternalLink } from "../api";
import Icon from "./icons/Icon.vue";

const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();

const version = ref("");
const titleId = `about-modal-title-${Math.random().toString(36).slice(2)}`;
const REPO_URL = "https://github.com/jagjitphull/notes";

let previouslyFocused: HTMLElement | null = null;

onMounted(async () => {
  previouslyFocused = document.activeElement as HTMLElement | null;
  try {
    version.value = await getVersion();
  } catch {
    // Not fatal - the rest of the dialog is still useful without it.
  }
});

onBeforeUnmount(() => {
  previouslyFocused?.focus();
});

function openRepo() {
  openExternalLink(REPO_URL).catch((e) => console.error("failed to open link", e));
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-backdrop" @click.self="emit('close')" @keydown.esc="emit('close')">
      <div class="modal" role="dialog" aria-modal="true" :aria-labelledby="titleId" tabindex="-1">
        <Icon class="about-icon" name="notesList" />
        <h2 :id="titleId">{{ t('common.appName') }}</h2>
        <p v-if="version" class="about-version">{{ t('about.version', { version }) }}</p>
        <p class="about-description">{{ t('about.description') }}</p>
        <button type="button" class="about-link" @click="openRepo">
          <Icon name="link" />
          {{ t('about.sourceLink') }}
        </button>
        <p class="about-license">{{ t('about.license') }}</p>
        <div class="modal-actions">
          <button type="button" class="primary" autofocus @click="emit('close')">
            {{ t('common.close') }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1100;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal {
  width: 300px;
  background: var(--bg-list);
  border-radius: 10px;
  padding: 24px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.3);
  text-align: center;
}

.about-icon {
  width: 40px;
  height: 40px;
  color: var(--accent-blue);
  margin-bottom: 8px;
}

.modal h2 {
  font-size: 17px;
  font-weight: 700;
  margin: 0 0 4px;
  color: var(--text-primary);
}

.about-version {
  font-size: 12.5px;
  color: var(--text-tertiary);
  margin: 0 0 14px;
}

.about-description {
  font-size: 13px;
  color: var(--text-secondary);
  margin: 0 0 16px;
  line-height: 1.5;
}

.about-link {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: transparent;
  color: var(--accent-blue-text);
  font: inherit;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  padding: 4px 0;
}

.about-link svg {
  width: 14px;
  height: 14px;
}

.about-link:hover {
  text-decoration: underline;
}

.about-license {
  font-size: 12px;
  color: var(--text-tertiary);
  margin: 14px 0 0;
}

.modal-actions {
  display: flex;
  justify-content: center;
  margin-top: 18px;
}

.modal-actions .primary {
  padding: 6px 20px;
  border-radius: 7px;
  border: none;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  background: var(--bg-selected-strong);
  color: #fff;
}
</style>
