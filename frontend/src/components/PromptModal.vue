<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  title: string;
  initialValue?: string;
  confirmLabel?: string;
}>();

const emit = defineEmits<{ confirm: [value: string]; cancel: [] }>();

const { t } = useI18n();

const value = ref(props.initialValue ?? "");
const inputRef = ref<HTMLInputElement | null>(null);
const formRef = ref<HTMLFormElement | null>(null);
const titleId = `prompt-modal-title-${Math.random().toString(36).slice(2)}`;

let previouslyFocused: HTMLElement | null = null;

onMounted(async () => {
  previouslyFocused = document.activeElement as HTMLElement | null;
  await nextTick();
  inputRef.value?.focus();
  inputRef.value?.select();
});

// Dialogs must trap Tab within themselves (WAI-ARIA APG) - otherwise
// keyboard/screen-reader users can Tab straight into the app behind it
// while the modal is still open.
function onKeydownTab(e: KeyboardEvent) {
  if (e.key !== "Tab" || !formRef.value) return;
  const focusable = formRef.value.querySelectorAll<HTMLElement>(
    "input, button:not(:disabled)",
  );
  if (focusable.length === 0) return;
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

onBeforeUnmount(() => {
  previouslyFocused?.focus();
});

function submit() {
  const trimmed = value.value.trim();
  if (trimmed) emit("confirm", trimmed);
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-backdrop" @click.self="emit('cancel')" @keydown.esc="emit('cancel')">
      <form
        ref="formRef"
        class="modal"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="titleId"
        @submit.prevent="submit"
        @keydown="onKeydownTab"
      >
        <h2 :id="titleId">{{ title }}</h2>
        <input ref="inputRef" v-model="value" type="text" @keydown.esc="emit('cancel')" />
        <div class="modal-actions">
          <button type="button" class="secondary" @click="emit('cancel')">{{ t('common.cancel') }}</button>
          <button type="submit" class="primary" :disabled="!value.trim()">
            {{ confirmLabel ?? t('common.ok') }}
          </button>
        </div>
      </form>
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
  width: 320px;
  background: var(--bg-list);
  border-radius: 10px;
  padding: 18px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.3);
}

.modal h2 {
  font-size: 14px;
  font-weight: 600;
  margin: 0 0 10px;
  color: var(--text-primary);
}

.modal input {
  width: 100%;
  box-sizing: border-box;
  padding: 7px 9px;
  border-radius: 6px;
  border: 1px solid var(--border);
  background: var(--bg-editor);
  color: var(--text-primary);
  font: inherit;
  font-size: 13.5px;
  outline: none;
}

.modal input:focus {
  border-color: var(--accent-blue);
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

.modal-actions .primary {
  background: var(--bg-selected-strong);
  color: #fff;
}

.modal-actions .primary:disabled {
  opacity: 0.5;
  cursor: default;
}

.modal-actions .secondary {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
