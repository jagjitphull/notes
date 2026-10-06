<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { unlockVaultWithPassword, unlockVaultWithRecoveryKey } from "../api";

const emit = defineEmits<{ unlocked: [] }>();

const { t } = useI18n();

const mode = ref<"password" | "recovery">("password");
const input = ref("");
const busy = ref(false);
const error = ref<string | null>(null);
const inputRef = ref<HTMLInputElement | null>(null);

async function focusInput() {
  await nextTick();
  inputRef.value?.focus();
}

onMounted(focusInput);
watch(mode, () => {
  input.value = "";
  error.value = null;
  focusInput();
});

async function submit() {
  if (!input.value || busy.value) return;
  busy.value = true;
  error.value = null;
  try {
    if (mode.value === "password") {
      await unlockVaultWithPassword(input.value);
    } else {
      await unlockVaultWithRecoveryKey(input.value);
    }
    emit("unlocked");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="setup-screen">
    <div class="setup-card">
      <h1>{{ t('vault.unlockHeading') }}</h1>
      <p class="subtitle">
        {{ mode === 'password' ? t('vault.unlockSubtitlePassword') : t('vault.unlockSubtitleRecovery') }}
      </p>

      <form class="unlock-form" @submit.prevent="submit">
        <input
          ref="inputRef"
          v-model="input"
          :type="mode === 'password' ? 'password' : 'text'"
          :placeholder="mode === 'password' ? t('vault.passwordPlaceholder') : t('vault.recoveryKeyPlaceholder')"
          :disabled="busy"
          autocomplete="off"
        />
        <button class="primary" type="submit" :disabled="busy || !input">
          {{ t('vault.unlockButton') }}
        </button>
      </form>

      <button class="link-button" type="button" @click="mode = mode === 'password' ? 'recovery' : 'password'">
        {{ mode === 'password' ? t('vault.useRecoveryKeyInstead') : t('vault.usePasswordInstead') }}
      </button>

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
  max-width: 400px;
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

.unlock-form {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.unlock-form input {
  padding: 9px 12px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-list);
  color: var(--text-primary);
  font: inherit;
  font-size: 13.5px;
  outline: none;
  text-align: center;
}

.unlock-form input:focus {
  border-color: var(--accent-blue);
}

button.primary {
  padding: 9px 16px;
  border-radius: 8px;
  border: none;
  font-size: 13.5px;
  font-weight: 600;
  cursor: pointer;
  background: var(--bg-selected-strong);
  color: #fff;
}

button.primary:disabled {
  opacity: 0.6;
  cursor: default;
}

.link-button {
  margin-top: 16px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--accent-blue-text);
  font-size: 12.5px;
  cursor: pointer;
}

.link-button:hover {
  text-decoration: underline;
}

.error {
  margin-top: 16px;
  font-size: 12.5px;
  color: var(--danger);
}
</style>
