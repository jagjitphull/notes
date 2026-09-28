<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";

export interface ContextMenuItem {
  label: string;
  action: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
}

const props = defineProps<{
  x: number;
  y: number;
  items: ContextMenuItem[];
}>();

const emit = defineEmits<{ close: [] }>();

const menuRef = ref<HTMLUListElement | null>(null);

// Clamp so the menu never renders off the right/bottom edge of the window.
const style = ref({ left: `${props.x}px`, top: `${props.y}px` });

onMounted(() => {
  const el = menuRef.value;
  if (el) {
    const rect = el.getBoundingClientRect();
    const maxLeft = window.innerWidth - rect.width - 8;
    const maxTop = window.innerHeight - rect.height - 8;
    style.value = {
      left: `${Math.max(8, Math.min(props.x, maxLeft))}px`,
      top: `${Math.max(8, Math.min(props.y, maxTop))}px`,
    };
  }
  document.addEventListener("keydown", onKeydown);
});

onUnmounted(() => {
  document.removeEventListener("keydown", onKeydown);
});

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

function run(item: ContextMenuItem) {
  if (item.disabled) return;
  item.action();
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <div class="menu-backdrop" @click="emit('close')" @contextmenu.prevent="emit('close')">
      <ul ref="menuRef" class="menu" :style="style" @click.stop @contextmenu.stop.prevent>
        <template v-for="(item, i) in items" :key="i">
          <li v-if="item.separator" class="menu-separator" />
          <li v-else>
            <button
              class="menu-item"
              :class="{ danger: item.danger }"
              :disabled="item.disabled"
              @click="run(item)"
            >
              {{ item.label }}
            </button>
          </li>
        </template>
      </ul>
    </div>
  </Teleport>
</template>

<style scoped>
.menu-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
}

.menu {
  position: fixed;
  min-width: 190px;
  max-width: 280px;
  background: var(--bg-list);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
  padding: 4px;
  list-style: none;
  margin: 0;
}

.menu-item {
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

.menu-item:hover:not(:disabled) {
  background: var(--bg-hover);
}

.menu-item.danger {
  color: var(--danger);
}

.menu-item:disabled {
  opacity: 0.4;
  cursor: default;
}

.menu-separator {
  height: 1px;
  background: var(--border);
  margin: 4px 6px;
}
</style>
