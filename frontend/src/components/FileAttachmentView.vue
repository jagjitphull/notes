<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { NodeViewWrapper, nodeViewProps } from "@tiptap/vue-3";
import Icon from "./icons/Icon.vue";
import { getAttachmentSize, openAttachment, readAttachmentBase64, readAttachmentText, saveAttachment } from "../api";
import { currentNoteId } from "../composables/useCurrentNoteId";
import type { Shape } from "../tiptap/drawingShapes";
import DrawingCanvas from "./DrawingCanvas.vue";

const props = defineProps(nodeViewProps);
const { t } = useI18n();

const path = computed(() => (props.node.attrs.path as string) ?? "");
const filename = computed(() => path.value.split("/").pop() || path.value);
// DrawingCanvas.vue saves a drawing's two halves under this exact suffix
// convention (see Editor.vue's onDrawingSave) - nothing else in the app
// writes a "*.drawing.png", so this is a safe, unambiguous way to tell a
// drawing embed apart from a dropped-in image/document without a second
// node type or attribute.
const isDrawing = computed(() => filename.value.endsWith(".drawing.png"));
const sidecarPath = computed(() => path.value.replace(/\.png$/, ".json"));

// Always re-stat on mount rather than trusting a size baked into the
// node/markdown at insert time - the file on disk is the single source of
// truth, and this way a reopened note shows the same thing a freshly
// inserted one does, with no separate "did this go stale" question.
const size = ref<number | null>(null);
const broken = ref(false);
const imageSrc = ref<string | null>(null);
const editorOpen = ref(false);

// This node view can be torn down (note switched, content resynced)
// while one of the functions below is mid-`await` - mutating its reactive
// state after that happens is a no-op at best, and at worst races the
// node view renderer's own re-render of the same component instance.
let isMounted = true;
onUnmounted(() => {
  isMounted = false;
});

onMounted(async () => {
  try {
    if (isDrawing.value) {
      const base64 = await readAttachmentBase64(path.value);
      if (!isMounted) return;
      imageSrc.value = `data:image/png;base64,${base64}`;
    } else {
      const bytes = await getAttachmentSize(path.value);
      if (!isMounted) return;
      size.value = bytes;
    }
  } catch {
    if (isMounted) broken.value = true;
  }
});

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function onClick() {
  if (broken.value) return;
  openAttachment(path.value);
}

const initialShapes = ref<Shape[] | undefined>(undefined);

async function openEditor() {
  let shapes: Shape[];
  try {
    shapes = JSON.parse(await readAttachmentText(sidecarPath.value));
  } catch {
    shapes = [];
  }
  if (!isMounted) return;
  initialShapes.value = shapes;
  editorOpen.value = true;
}

async function onEditorSave({ png, shapes }: { png: Uint8Array; shapes: Shape[] }) {
  editorOpen.value = false;
  const noteId = currentNoteId.value;
  if (!noteId) return;
  const uuid = crypto.randomUUID();
  try {
    const pngInfo = await saveAttachment(noteId, `${uuid}.drawing.png`, png);
    const jsonBytes = new TextEncoder().encode(JSON.stringify(shapes));
    await saveAttachment(noteId, `${uuid}.drawing.json`, jsonBytes);
    if (!isMounted) return;
    props.updateAttributes({ path: pngInfo.path });
    const base64 = await readAttachmentBase64(pngInfo.path);
    if (!isMounted) return;
    imageSrc.value = `data:image/png;base64,${base64}`;
    broken.value = false;
  } catch (e) {
    console.error("failed to save re-edited drawing", e);
  }
}
</script>

<template>
  <NodeViewWrapper v-if="isDrawing" as="span" class="drawing-embed" :class="{ 'drawing-embed--broken': broken }">
    <img v-if="imageSrc" :src="imageSrc" :alt="t('drawing.title')" />
    <span v-else-if="broken" class="drawing-broken">{{ t('drawing.broken') }}</span>
    <button
      v-if="!broken"
      type="button"
      class="drawing-edit-button"
      :title="t('drawing.edit')"
      :aria-label="t('drawing.edit')"
      @mousedown.prevent.stop
      @click.stop="openEditor"
    >
      <Icon name="pencil" />
    </button>
    <DrawingCanvas v-if="editorOpen" :initial-shapes="initialShapes" @save="onEditorSave" @cancel="editorOpen = false" />
  </NodeViewWrapper>

  <NodeViewWrapper
    v-else
    as="span"
    class="file-attachment"
    :class="{ 'file-attachment--broken': broken }"
    :title="filename"
    @click="onClick"
  >
    <Icon name="attachment" class="file-attachment-icon" />
    <span class="file-attachment-name">{{ filename }}</span>
    <span v-if="size !== null" class="file-attachment-size">{{ formatSize(size) }}</span>
  </NodeViewWrapper>
</template>

<style scoped>
.drawing-embed {
  position: relative;
  display: inline-block;
  max-width: 100%;
  vertical-align: middle;
  line-height: 0;
}

.drawing-embed img {
  max-width: 100%;
  border-radius: 6px;
  border: 1px solid var(--border);
  display: block;
}

.drawing-broken {
  display: inline-block;
  padding: 10px 14px;
  border: 1px dashed var(--border);
  border-radius: 6px;
  color: var(--text-tertiary);
  font-size: 0.9em;
}

.drawing-edit-button {
  position: absolute;
  top: 6px;
  right: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 6px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.12s ease;
}

.drawing-embed:hover .drawing-edit-button,
.drawing-edit-button:focus-visible {
  opacity: 1;
}

.drawing-edit-button svg {
  width: 14px;
  height: 14px;
}

.file-attachment {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  border-radius: 6px;
  padding: 2px 8px 2px 6px;
  color: var(--text-primary);
  background: var(--bg-selected);
  border: 1px solid var(--border);
  cursor: pointer;
  font-size: 0.92em;
  vertical-align: middle;
}

.file-attachment:hover {
  background: var(--bg-hover);
}

.file-attachment-icon {
  width: 13px;
  height: 13px;
  flex: 0 0 auto;
  color: var(--text-secondary);
}

.file-attachment-name {
  max-width: 240px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-attachment-size {
  color: var(--text-tertiary);
  font-size: 0.85em;
}

.file-attachment--broken {
  color: var(--text-tertiary);
  background: transparent;
  border-style: dashed;
  cursor: default;
}
</style>
