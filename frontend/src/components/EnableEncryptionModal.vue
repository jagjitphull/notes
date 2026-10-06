<script setup lang="ts">
import { nextTick, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { enableVaultEncryption } from "../api";

const emit = defineEmits<{ close: []; enabled: [] }>();

const { t } = useI18n();

const step = ref<"password" | "recovery">("password");
const password = ref("");
const confirmPassword = ref("");
const recoveryKey = ref("");
const busy = ref(false);
const error = ref<string | null>(null);
const savedConfirmed = ref(false);
const copied = ref(false);
const passwordInputRef = ref<HTMLInputElement | null>(null);

onMounted(async () => {
  await nextTick();
  passwordInputRef.value?.focus();
});

async function submitPassword() {
  if (busy.value) return;
  error.value = null;
  if (!password.value) {
    error.value = t("vault.errorPasswordRequired");
    return;
  }
  if (password.value !== confirmPassword.value) {
    error.value = t("vault.errorPasswordsDontMatch");
    return;
  }
  busy.value = true;
  try {
    recoveryKey.value = await enableVaultEncryption(password.value);
    step.value = "recovery";
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function copyRecoveryKey() {
  try {
    await navigator.clipboard.writeText(recoveryKey.value);
    copied.value = true;
  } catch {
    // Clipboard access can fail silently (e.g. no permission) - the key
    // is still fully visible and selectable on screen either way.
  }
}

function finish() {
  emit("enabled");
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-backdrop" @click.self="step === 'password' && emit('close')">
      <div class="modal" role="dialog" aria-modal="true">
        <template v-if="step === 'password'">
          <h2>{{ t('vault.enableTitle') }}</h2>
          <p class="modal-subtitle">{{ t('vault.enableSubtitle') }}</p>
          <form @submit.prevent="submitPassword">
            <input
              ref="passwordInputRef"
              v-model="password"
              type="password"
              :placeholder="t('vault.passwordPlaceholder')"
              :disabled="busy"
              autocomplete="new-password"
            />
            <input
              v-model="confirmPassword"
              type="password"
              :placeholder="t('vault.confirmPasswordPlaceholder')"
              :disabled="busy"
              autocomplete="new-password"
            />
            <p v-if="error" class="error" role="alert">{{ error }}</p>
            <div class="modal-actions">
              <button type="button" class="secondary" :disabled="busy" @click="emit('close')">
                {{ t('common.cancel') }}
              </button>
              <button type="submit" class="primary" :disabled="busy || !password || !confirmPassword">
                {{ t('vault.enableButton') }}
              </button>
            </div>
          </form>
        </template>

        <template v-else>
          <h2>{{ t('vault.recoveryTitle') }}</h2>
          <p class="modal-subtitle">{{ t('vault.recoverySubtitle') }}</p>
          <div class="recovery-key">{{ recoveryKey }}</div>
          <button type="button" class="secondary copy-button" @click="copyRecoveryKey">
            {{ copied ? t('vault.copied') : t('vault.copyRecoveryKey') }}
          </button>
          <label class="confirm-checkbox">
            <input v-model="savedConfirmed" type="checkbox" />
            {{ t('vault.confirmSaved') }}
          </label>
          <div class="modal-actions">
            <button type="button" class="primary" :disabled="!savedConfirmed" @click="finish">
              {{ t('common.ok') }}
            </button>
          </div>
        </template>
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
  width: 360px;
  background: var(--bg-list);
  border-radius: 10px;
  padding: 20px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.3);
}

.modal h2 {
  font-size: 15px;
  font-weight: 600;
  margin: 0 0 6px;
  color: var(--text-primary);
}

.modal-subtitle {
  font-size: 12.5px;
  line-height: 1.5;
  color: var(--text-secondary);
  margin: 0 0 14px;
}

.modal input[type="password"] {
  width: 100%;
  box-sizing: border-box;
  padding: 8px 10px;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: var(--bg-editor);
  color: var(--text-primary);
  font: inherit;
  font-size: 13.5px;
  outline: none;
  margin-bottom: 8px;
}

.modal input[type="password"]:focus {
  border-color: var(--accent-blue);
}

.recovery-key {
  font-family: var(--mono, ui-monospace, monospace);
  font-size: 13px;
  line-height: 1.6;
  word-break: break-all;
  background: var(--bg-editor);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 10px 12px;
  color: var(--text-primary);
  user-select: all;
}

.copy-button {
  width: 100%;
  margin-top: 8px;
}

.confirm-checkbox {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
  font-size: 12.5px;
  color: var(--text-primary);
  cursor: pointer;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 14px;
}

.modal-actions button {
  padding: 6px 14px;
  border-radius: 7px;
  border: none;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.modal-actions .primary,
button.primary {
  background: var(--bg-selected-strong);
  color: #fff;
}

.modal-actions .primary:disabled,
button.primary:disabled {
  opacity: 0.5;
  cursor: default;
}

.modal-actions .secondary,
button.secondary {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.error {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--danger);
}
</style>
