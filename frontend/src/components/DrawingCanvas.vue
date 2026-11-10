<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import Icon from "./icons/Icon.vue";
import type { IconName } from "./icons/icons";
import type { Shape, Tool } from "../tiptap/drawingShapes";
import { drawShape } from "../tiptap/drawingShapes";

const props = defineProps<{
  initialShapes?: Shape[];
}>();

const emit = defineEmits<{
  save: [payload: { png: Uint8Array; shapes: Shape[] }];
  cancel: [];
}>();

const { t } = useI18n();

// A fixed drawing surface rather than an infinite/zoomable canvas - this
// is deliberately the "lightweight, Markdown-compatible" alternative to a
// true edgeless canvas (see the AFFiNE comparison this came out of):
// one bounded sketch per embed, rasterized to a plain PNG a note's own
// Markdown stays portable around.
const CANVAS_WIDTH = 640;
const CANVAS_HEIGHT = 400;

const COLORS = ["#1a1a1a", "#ff3b30", "#ff9500", "#ffcc00", "#34c759", "#0a84ff", "#af52de", "#8e8e93"] as const;
const WIDTHS = [2, 4, 7] as const;

const canvasRef = ref<HTMLCanvasElement | null>(null);
// Not structuredClone(): a re-edited drawing's initialShapes prop is a
// Vue-reactive value (sourced from a ref in the parent node view), and
// WebKitGTK's structuredClone - unlike V8's - refuses to clone a Proxy
// at all, even one wrapping plain, JSON-safe data. Shapes are plain JSON
// data with nothing structuredClone could do that JSON can't, so this
// sidesteps the engine difference rather than fighting it.
const shapes = ref<Shape[]>(JSON.parse(JSON.stringify(props.initialShapes ?? [])));
const redoStack = ref<Shape[]>([]);
const activeTool = ref<Tool>("pen");
const activeColor = ref<string>(COLORS[0]);
const activeWidth = ref<number>(WIDTHS[1]);

// The shape currently being drawn (pen stroke in progress, or a shape's
// drag-to-size preview) - kept separate from `shapes` until committed on
// pointerup, so a redraw can show it live without polluting undo history.
// Narrowed to exclude "text": the text tool never sets this ref (it goes
// through textInput.value instead, see onPointerDown), but that invariant
// isn't visible to the type checker from Shape alone.
type PendingShape = Extract<Shape, { type: "path" }> | Extract<Shape, { type: "rect" | "ellipse" | "line" }>;
const pending = ref<PendingShape | null>(null);

const textInput = ref<{ x: number; y: number; value: string } | null>(null);
const textInputRef = ref<HTMLInputElement | null>(null);

let previouslyFocused: HTMLElement | null = null;

function redraw() {
  const canvas = canvasRef.value;
  const ctx = canvas?.getContext("2d");
  if (!canvas || !ctx) return;
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  for (const shape of shapes.value) drawShape(ctx, shape);
  if (pending.value) drawShape(ctx, pending.value);
}

onMounted(() => {
  previouslyFocused = document.activeElement as HTMLElement | null;
  const canvas = canvasRef.value;
  if (canvas) {
    // Crisp strokes on a HiDPI display - the element's CSS size stays
    // CANVAS_WIDTH/HEIGHT, only the backing bitmap (and every coordinate
    // drawn to it) scales with devicePixelRatio.
    const ratio = window.devicePixelRatio || 1;
    canvas.width = CANVAS_WIDTH * ratio;
    canvas.height = CANVAS_HEIGHT * ratio;
    canvas.getContext("2d")?.scale(ratio, ratio);
  }
  redraw();
});

onBeforeUnmount(() => {
  previouslyFocused?.focus();
});

function pointFromEvent(e: PointerEvent): [number, number] {
  const rect = canvasRef.value!.getBoundingClientRect();
  return [e.clientX - rect.left, e.clientY - rect.top];
}

function hitShape(x: number, y: number): number {
  // Last-drawn-first, so erasing picks the shape actually on top.
  for (let i = shapes.value.length - 1; i >= 0; i--) {
    if (shapeContainsPoint(shapes.value[i], x, y)) return i;
  }
  return -1;
}

function shapeContainsPoint(shape: Shape, x: number, y: number): boolean {
  const near = (px: number, py: number, tolerance = 8) => Math.hypot(px - x, py - y) <= tolerance;
  switch (shape.type) {
    case "path":
      return shape.points.some(([px, py]) => near(px, py));
    case "line":
    case "rect":
    case "ellipse": {
      const minX = Math.min(shape.x1, shape.x2) - 6;
      const maxX = Math.max(shape.x1, shape.x2) + 6;
      const minY = Math.min(shape.y1, shape.y2) - 6;
      const maxY = Math.max(shape.y1, shape.y2) + 6;
      return x >= minX && x <= maxX && y >= minY && y <= maxY;
    }
    case "text":
      return x >= shape.x - 4 && x <= shape.x + shape.text.length * shape.fontSize * 0.6 && y <= shape.y + 4 && y >= shape.y - shape.fontSize - 4;
  }
}

let dragStart: [number, number] | null = null;

function onPointerDown(e: PointerEvent) {
  if (textInput.value) commitTextInput();
  const [x, y] = pointFromEvent(e);

  if (activeTool.value === "eraser") {
    const index = hitShape(x, y);
    if (index !== -1) {
      shapes.value.splice(index, 1);
      redoStack.value = [];
      redraw();
    }
    return;
  }

  if (activeTool.value === "text") {
    textInput.value = { x, y, value: "" };
    nextTick(() => textInputRef.value?.focus());
    return;
  }

  (e.target as HTMLElement).setPointerCapture(e.pointerId);
  dragStart = [x, y];
  const tool = activeTool.value;
  if (tool === "pen") {
    pending.value = { type: "path", points: [[x, y]], color: activeColor.value, width: activeWidth.value };
  } else {
    // tool is narrowed to "rect" | "ellipse" | "line" here - "text" and
    // "eraser" both return above before reaching this branch.
    pending.value = { type: tool, x1: x, y1: y, x2: x, y2: y, color: activeColor.value, width: activeWidth.value };
  }
}

function onPointerMove(e: PointerEvent) {
  // A fresh local binding, not a second `pending.value` read - Vue's ref
  // getter doesn't carry the type narrowing below across separate
  // accesses the way a plain variable does.
  const current = pending.value;
  if (!current || !dragStart) return;
  const [x, y] = pointFromEvent(e);
  if (current.type === "path") {
    current.points.push([x, y]);
  } else {
    current.x2 = x;
    current.y2 = y;
  }
  redraw();
}

function onPointerUp() {
  if (!pending.value) return;
  shapes.value.push(pending.value);
  redoStack.value = [];
  pending.value = null;
  dragStart = null;
  redraw();
}

function commitTextInput() {
  const pending = textInput.value;
  textInput.value = null;
  const text = pending?.value.trim();
  if (!pending || !text) return;
  shapes.value.push({ type: "text", x: pending.x, y: pending.y, text, color: activeColor.value, fontSize: 18 });
  redoStack.value = [];
  redraw();
}

function undo() {
  const last = shapes.value.pop();
  if (last) {
    redoStack.value.push(last);
    redraw();
  }
}

function redo() {
  const next = redoStack.value.pop();
  if (next) {
    shapes.value.push(next);
    redraw();
  }
}

function clearAll() {
  if (shapes.value.length === 0) return;
  if (!window.confirm(t("drawing.confirmClear"))) return;
  shapes.value = [];
  redoStack.value = [];
  redraw();
}

const canUndo = computed(() => shapes.value.length > 0);
const canRedo = computed(() => redoStack.value.length > 0);

const tools: { id: Tool; icon: IconName; labelKey: string }[] = [
  { id: "pen", icon: "pencil", labelKey: "drawing.tool.pen" },
  { id: "rect", icon: "shapeRectangle", labelKey: "drawing.tool.rectangle" },
  { id: "ellipse", icon: "shapeCircle", labelKey: "drawing.tool.ellipse" },
  { id: "line", icon: "shapeLine", labelKey: "drawing.tool.line" },
  { id: "text", icon: "textTool", labelKey: "drawing.tool.text" },
  { id: "eraser", icon: "eraser", labelKey: "drawing.tool.eraser" },
];

function save() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  canvas.toBlob(async (blob) => {
    if (!blob) return;
    const png = new Uint8Array(await blob.arrayBuffer());
    emit("save", { png, shapes: shapes.value });
  }, "image/png");
}
</script>

<template>
  <Teleport to="body">
    <div class="modal-backdrop" @keydown.esc="emit('cancel')">
      <div class="modal" role="dialog" aria-modal="true" :aria-label="t('drawing.title')" tabindex="-1">
        <div class="drawing-toolbar">
          <div class="tool-group">
            <button
              v-for="tool in tools"
              :key="tool.id"
              type="button"
              class="icon-button"
              :class="{ active: activeTool === tool.id }"
              :title="t(tool.labelKey)"
              :aria-label="t(tool.labelKey)"
              :aria-pressed="activeTool === tool.id"
              @click="activeTool = tool.id"
            >
              <Icon :name="tool.icon" />
            </button>
          </div>

          <div class="tool-group">
            <button
              v-for="color in COLORS"
              :key="color"
              type="button"
              class="color-swatch"
              :class="{ active: activeColor === color }"
              :style="{ backgroundColor: color }"
              :aria-label="t('drawing.color', { color })"
              :aria-pressed="activeColor === color"
              @click="activeColor = color"
            />
          </div>

          <div class="tool-group">
            <button
              v-for="width in WIDTHS"
              :key="width"
              type="button"
              class="width-swatch"
              :class="{ active: activeWidth === width }"
              :aria-label="t('drawing.strokeWidth', { width })"
              :aria-pressed="activeWidth === width"
              @click="activeWidth = width"
            >
              <span :style="{ height: `${width}px` }" />
            </button>
          </div>

          <div class="tool-group">
            <button
              type="button"
              class="icon-button"
              :disabled="!canUndo"
              :title="t('drawing.undo')"
              :aria-label="t('drawing.undo')"
              @click="undo"
            >
              <Icon name="restore" />
            </button>
            <button
              type="button"
              class="icon-button"
              :disabled="!canRedo"
              :title="t('drawing.redo')"
              :aria-label="t('drawing.redo')"
              @click="redo"
            >
              <Icon name="redo" />
            </button>
            <button
              type="button"
              class="icon-button"
              :disabled="shapes.length === 0"
              :title="t('drawing.clear')"
              :aria-label="t('drawing.clear')"
              @click="clearAll"
            >
              <Icon name="trash" />
            </button>
          </div>
        </div>

        <div class="canvas-wrap" :style="{ width: `${CANVAS_WIDTH}px`, height: `${CANVAS_HEIGHT}px` }">
          <canvas
            ref="canvasRef"
            :style="{ width: `${CANVAS_WIDTH}px`, height: `${CANVAS_HEIGHT}px`, cursor: activeTool === 'eraser' ? 'cell' : 'crosshair' }"
            @pointerdown="onPointerDown"
            @pointermove="onPointerMove"
            @pointerup="onPointerUp"
            @pointerleave="onPointerUp"
          />
          <input
            v-if="textInput"
            ref="textInputRef"
            v-model="textInput.value"
            class="text-input"
            :style="{ left: `${textInput.x}px`, top: `${textInput.y - 18}px`, color: activeColor }"
            @keydown.enter="commitTextInput"
            @keydown.esc="textInput = null"
            @blur="commitTextInput"
          />
        </div>

        <div class="modal-actions">
          <button type="button" class="secondary" @click="emit('cancel')">{{ t("common.cancel") }}</button>
          <button type="button" class="primary" @click="save">{{ t("common.ok") }}</button>
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
  background: var(--bg-list);
  border-radius: 10px;
  padding: 14px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.3);
}

.drawing-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  margin-bottom: 10px;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border);
}

.tool-group {
  display: flex;
  align-items: center;
  gap: 4px;
  padding-right: 10px;
  border-right: 1px solid var(--border);
}

.tool-group:last-child {
  padding-right: 0;
  border-right: none;
}

.icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.icon-button svg {
  width: 16px;
  height: 16px;
}

.icon-button:hover:not(:disabled) {
  background: var(--bg-hover);
}

.icon-button.active {
  background: var(--bg-selected-strong);
  color: #fff;
}

.icon-button:disabled {
  opacity: 0.35;
  cursor: default;
}

.color-swatch {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  padding: 0;
}

.color-swatch.active {
  border-color: var(--text-primary);
}

.width-swatch {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
}

.width-swatch span {
  display: block;
  width: 14px;
  border-radius: 2px;
  background: var(--text-primary);
}

.width-swatch:hover,
.width-swatch.active {
  background: var(--bg-hover);
}

.canvas-wrap {
  position: relative;
  border: 1px solid var(--border);
  border-radius: 6px;
  overflow: hidden;
}

.canvas-wrap canvas {
  display: block;
  touch-action: none;
}

.text-input {
  position: absolute;
  font-size: 18px;
  font-family: inherit;
  border: 1px dashed var(--accent-blue);
  background: rgba(255, 255, 255, 0.85);
  padding: 1px 3px;
  outline: none;
  min-width: 40px;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
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

.modal-actions .secondary {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
