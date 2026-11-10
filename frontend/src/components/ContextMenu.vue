<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import Icon from "./icons/Icon.vue";

export interface ContextMenuSwatch {
  color: string | null;
  label: string;
  selected: boolean;
  action: () => void;
}

export interface ContextMenuItem {
  label: string;
  action: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  checked?: boolean;
  // When set, this entry renders as a row of color swatches instead of a
  // normal label/action button - label/action/checked are unused for it.
  swatches?: ContextMenuSwatch[];
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

let previouslyFocused: HTMLElement | null = null;

function menuItemEls(): HTMLElement[] {
  return Array.from(
    menuRef.value?.querySelectorAll<HTMLElement>(
      ".menu-item:not(:disabled), .menu-swatch",
    ) ?? [],
  );
}

onMounted(() => {
  previouslyFocused = document.activeElement as HTMLElement | null;

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
  // Move focus into the menu so keyboard/screen-reader users land
  // somewhere sensible instead of the trigger element underneath.
  menuItemEls()[0]?.focus();
});

onUnmounted(() => {
  document.removeEventListener("keydown", onKeydown);
  previouslyFocused?.focus();
});

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") {
    emit("close");
    return;
  }
  if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
  const items = menuItemEls();
  if (items.length === 0) return;
  e.preventDefault();
  const currentIndex = items.indexOf(document.activeElement as HTMLElement);
  const delta = e.key === "ArrowDown" ? 1 : -1;
  const nextIndex = (currentIndex + delta + items.length) % items.length;
  items[nextIndex]?.focus();
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
      <ul ref="menuRef" class="menu" role="menu" :style="style" @click.stop @contextmenu.stop.prevent>
        <template v-for="(item, i) in items" :key="i">
          <li v-if="item.separator" class="menu-separator" role="separator" />
          <li v-else-if="item.swatches" class="menu-swatches" role="none">
            <button
              v-for="swatch in item.swatches"
              :key="swatch.label"
              class="menu-swatch"
              role="menuitemradio"
              :aria-checked="swatch.selected"
              :aria-label="swatch.label"
              :title="swatch.label"
              :style="swatch.color ? { background: swatch.color } : undefined"
              :class="{ 'menu-swatch--none': !swatch.color }"
              @click="
                swatch.action();
                emit('close');
              "
            >
              <Icon v-if="swatch.selected" name="check" />
            </button>
          </li>
          <li v-else role="none">
            <button
              class="menu-item"
              role="menuitem"
              :class="{ danger: item.danger }"
              :disabled="item.disabled"
              @click="run(item)"
            >
              <Icon v-if="item.checked" name="check" class="menu-item-check" />
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

.menu-item-check {
  width: 14px;
  height: 14px;
  margin-right: 6px;
  vertical-align: -2px;
}

.menu-swatches {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 6px 10px;
}

.menu-swatch {
  width: 22px;
  height: 22px;
  padding: 0;
  border-radius: 50%;
  border: 1px solid var(--border);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #fff;
}

.menu-swatch svg {
  width: 12px;
  height: 12px;
}

.menu-swatch--none {
  background: transparent;
  color: var(--text-primary);
}

.menu-swatch:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
</style>
