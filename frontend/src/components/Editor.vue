<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, watch } from "vue";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Placeholder from "@tiptap/extension-placeholder";
import Image from "@tiptap/extension-image";
import HighlightBase from "@tiptap/extension-highlight";
import { Markdown } from "tiptap-markdown";
import markdownItMark from "markdown-it-mark";
import type { Folder, Note, Tag } from "../types";

// Round-trips through the "==highlighted==" markdown-it-mark convention
// (same syntax Obsidian and others use), since tiptap-markdown has no
// built-in spec for this mark and CommonMark has no native highlight syntax.
const Highlight = HighlightBase.extend({
  addStorage() {
    return {
      markdown: {
        serialize: { open: "==", close: "==" },
        parse: {
          setup(markdownit: import("markdown-it")) {
            markdownit.use(markdownItMark);
          },
        },
      },
    };
  },
});

const props = defineProps<{
  note: Note | null;
  folders: Folder[];
  tags: Tag[];
}>();

const emit = defineEmits<{
  togglePin: [];
  toggleDeleted: [];
  addTag: [];
  removeTag: [name: string];
}>();

const noteTagNames = computed(
  () =>
    props.tags
      .filter((t) => props.note?.tagIds.includes(t.id))
      .map((t) => t.name)
      .sort(),
);

const body = defineModel<string>("body", { default: "" });

const isDeleted = computed(() => !!props.note?.deletedAt);

const editor = useEditor({
  content: "",
  editable: !isDeleted.value,
  extensions: [
    StarterKit.configure({ heading: { levels: [1, 2, 3] } }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Placeholder.configure({ placeholder: "New Note" }),
    Image,
    Highlight,
    Markdown.configure({
      html: false,
      tightLists: true,
      linkify: true,
      // Treat a bare newline as a hard line break (like a plain text editor)
      // rather than folding it into the surrounding paragraph per strict
      // CommonMark. This matters for loading notes that predate the rich
      // editor (or any externally-authored file): without it, a plain
      // multi-line note collapses into one run-on paragraph the first time
      // it's opened here.
      breaks: true,
    }),
  ],
  editorProps: {
    handleDrop(view, event) {
      const files = Array.from(event.dataTransfer?.files ?? []).filter((f) =>
        f.type.startsWith("image/"),
      );
      if (files.length === 0) return false;
      event.preventDefault();

      const pos = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos;
      for (const file of files) {
        const reader = new FileReader();
        reader.onload = () => {
          const node = view.state.schema.nodes.image.create({ src: reader.result });
          const tr = view.state.tr.insert(pos ?? view.state.doc.content.size, node);
          view.dispatch(tr);
        };
        reader.readAsDataURL(file);
      }
      return true;
    },
  },
  onUpdate({ editor }) {
    body.value = editor.storage.markdown.getMarkdown();
  },
});

// Sync content into the editor whenever `body` diverges from what the
// editor itself last produced — i.e. it changed for an external reason
// (a different note was selected, or its body was freshly fetched). This
// naturally no-ops on the editor's own onUpdate-triggered writes, and is
// immune to load-order races since it just compares current values.
watch(body, (value) => {
  if (!editor.value) return;
  if (value !== editor.value.storage.markdown.getMarkdown()) {
    editor.value.commands.setContent(value, { emitUpdate: false });
  }
});

watch(isDeleted, (deleted) => {
  editor.value?.setEditable(!deleted);
});

onBeforeUnmount(() => {
  editor.value?.destroy();
});

// Deliberately not auto-focused on every note switch: that would steal
// keyboard focus into the (now-editable) content on every arrow-key list
// navigation step, breaking "arrow keys to navigate the notes list" after
// the very first step. The parent calls this explicitly right after
// creating a note, where jumping straight into typing is exactly wanted.
async function focusEditor() {
  await nextTick();
  editor.value?.commands.focus("end");
}

defineExpose({ focusEditor });

const folderName = computed(
  () => props.folders.find((f) => f.id === props.note?.folderId)?.name ?? "",
);

const formattedDate = computed(() => {
  if (!props.note) return "";
  return new Date(props.note.updatedAt).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
});

type ToolbarAction = {
  label: string;
  icon: string;
  isActive: () => boolean;
  run: () => void;
};

const toolbarActions = computed<ToolbarAction[]>(() => {
  const e = editor.value;
  if (!e) return [];
  const chain = () => e.chain().focus();
  return [
    { label: "Bold", icon: "B", isActive: () => e.isActive("bold"), run: () => chain().toggleBold().run() },
    { label: "Italic", icon: "I", isActive: () => e.isActive("italic"), run: () => chain().toggleItalic().run() },
    { label: "Underline", icon: "U", isActive: () => e.isActive("underline"), run: () => chain().toggleUnderline().run() },
    { label: "Strikethrough", icon: "S", isActive: () => e.isActive("strike"), run: () => chain().toggleStrike().run() },
    { label: "Highlight", icon: "✎", isActive: () => e.isActive("highlight"), run: () => chain().toggleHighlight().run() },
    { label: "Heading 1", icon: "H1", isActive: () => e.isActive("heading", { level: 1 }), run: () => chain().toggleHeading({ level: 1 }).run() },
    { label: "Heading 2", icon: "H2", isActive: () => e.isActive("heading", { level: 2 }), run: () => chain().toggleHeading({ level: 2 }).run() },
    { label: "Checklist", icon: "☑", isActive: () => e.isActive("taskList"), run: () => chain().toggleTaskList().run() },
    { label: "Bullet List", icon: "•", isActive: () => e.isActive("bulletList"), run: () => chain().toggleBulletList().run() },
    { label: "Numbered List", icon: "1.", isActive: () => e.isActive("orderedList"), run: () => chain().toggleOrderedList().run() },
    { label: "Code Block", icon: "</>", isActive: () => e.isActive("codeBlock"), run: () => chain().toggleCodeBlock().run() },
    { label: "Quote", icon: "”", isActive: () => e.isActive("blockquote"), run: () => chain().toggleBlockquote().run() },
  ];
});
</script>

<template>
  <main class="editor">
    <template v-if="note">
      <div class="editor-toolbar">
        <div class="format-bar">
          <button
            v-for="action in toolbarActions"
            :key="action.label"
            class="format-button"
            :class="{ active: action.isActive() }"
            :title="action.label"
            :disabled="isDeleted"
            @click="action.run"
          >
            {{ action.icon }}
          </button>
        </div>
        <div class="toolbar-right">
          <span class="editor-meta">{{ formattedDate }} &middot; {{ folderName }}</span>
          <button
            class="icon-button"
            :class="{ active: note.isPinned }"
            :title="note.isPinned ? 'Unpin' : 'Pin'"
            :disabled="isDeleted"
            @click="emit('togglePin')"
          >
            <svg viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
              <path d="M11.5 2.5a1 1 0 0 1 1.4 0l4.6 4.6a1 1 0 0 1 0 1.4l-.7.7a1 1 0 0 1-1.4 0l-.2-.2-2.6 2.6.6 2.9a.75.75 0 0 1-1.27.68l-2.9-2.9-4 4a.6.6 0 0 1-.85-.85l4-4-2.9-2.9a.75.75 0 0 1 .68-1.27l2.9.6 2.6-2.6-.2-.2a1 1 0 0 1 0-1.4l.7-.7Z" />
            </svg>
          </button>
          <button
            class="icon-button"
            :title="isDeleted ? 'Restore' : 'Delete'"
            @click="emit('toggleDeleted')"
          >
            <svg v-if="isDeleted" viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <path d="M4 10a6 6 0 1 1 2 4.5M4 10V6M4 10h4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <svg v-else viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <path
                d="M5 6.5h10M8.25 6.5V5a1 1 0 0 1 1-1h1.5a1 1 0 0 1 1 1v1.5M8.5 9.5v4M11.5 9.5v4M5.75 6.5l.6 8.1a1.5 1.5 0 0 0 1.496 1.4h4.308a1.5 1.5 0 0 0 1.496-1.4l.6-8.1"
                stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"
              />
            </svg>
          </button>
        </div>
      </div>

      <div v-if="!isDeleted" class="tags-row">
        <span v-for="name in noteTagNames" :key="name" class="tag-chip">
          #{{ name }}
          <button
            class="tag-remove"
            title="Remove tag"
            :aria-label="`Remove tag ${name}`"
            @click="emit('removeTag', name)"
          >
            &times;
          </button>
        </span>
        <button class="add-tag-button" title="Add Tag" @click="emit('addTag')">
          + Tag
        </button>
      </div>

      <div class="editor-canvas">
        <EditorContent :editor="editor" class="editor-content" />
      </div>
    </template>

    <div v-else class="empty-editor">
      <svg viewBox="0 0 48 48" fill="none" aria-hidden="true">
        <rect x="8" y="6" width="32" height="36" rx="4" stroke="currentColor" stroke-width="2" />
        <path d="M15 16h18M15 23h18M15 30h11" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
      </svg>
      <p>No Note Selected</p>
    </div>
  </main>
</template>

<style scoped>
.editor {
  height: 100%;
  /* Also a grid item (in App.vue's .app-shell) - see NoteList.vue's
     .note-list comment for why this needs min-height: 0 too. */
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg-editor);
}

.editor-toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--border);
}

.format-bar {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-wrap: wrap;
}

.format-button {
  min-width: 26px;
  height: 26px;
  padding: 0 6px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
}

.format-button:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.format-button.active {
  background: var(--bg-selected);
  color: var(--accent-blue);
}

.format-button:disabled {
  opacity: 0.4;
  cursor: default;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 4px;
  flex: 0 0 auto;
}

.editor-meta {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  margin-right: 4px;
}

.tags-row {
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 8px 48px 0;
  max-width: 760px;
  margin: 0 auto;
  width: 100%;
  box-sizing: border-box;
}

.tag-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 4px 2px 8px;
  border-radius: 12px;
  background: var(--bg-hover);
  color: var(--text-secondary);
  font-size: 12px;
}

.tag-remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-tertiary);
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
}

.tag-remove:hover {
  background: var(--border);
  color: var(--text-primary);
}

.add-tag-button {
  padding: 3px 9px;
  border: 1px dashed var(--border);
  border-radius: 12px;
  background: transparent;
  color: var(--text-tertiary);
  font-size: 12px;
  cursor: pointer;
}

.add-tag-button:hover {
  border-color: var(--accent-blue);
  color: var(--accent-blue);
}

.editor-canvas {
  flex: 1 1 auto;
  /* Flex items default to min-height: auto (content-based), which
     overrides flex-shrink and defeats overflow-y: auto - a long note
     would just grow the box instead of scrolling within it. */
  min-height: 0;
  overflow-y: auto;
  padding: 16px 48px 48px;
  max-width: 760px;
  margin: 0 auto;
  width: 100%;
}

.editor-content :deep(.ProseMirror) {
  outline: none;
  font-size: 15px;
  line-height: 1.6;
  color: var(--text-primary);
  min-height: 100%;
}

.editor-content :deep(.ProseMirror p.is-editor-empty:first-child::before) {
  content: attr(data-placeholder);
  float: left;
  color: var(--text-tertiary);
  pointer-events: none;
  height: 0;
}

.editor-content :deep(.ProseMirror > *:first-child) {
  margin-top: 0;
}

/* Only the visual first line reads as a title, matching Apple Notes —
   even when hard breaks (see the `breaks` Markdown option) put more than
   one line inside that first block. */
.editor-content :deep(.ProseMirror > *:first-child::first-line) {
  font-size: 24px;
  font-weight: 700;
}

.editor-content :deep(.ProseMirror h1) {
  font-size: 22px;
  font-weight: 700;
  margin: 18px 0 6px;
}

.editor-content :deep(.ProseMirror h2) {
  font-size: 18px;
  font-weight: 700;
  margin: 16px 0 6px;
}

.editor-content :deep(.ProseMirror p) {
  margin: 0 0 8px;
}

.editor-content :deep(.ProseMirror ul),
.editor-content :deep(.ProseMirror ol) {
  padding-left: 22px;
  margin: 0 0 8px;
}

.editor-content :deep(.ProseMirror ul[data-type="taskList"]) {
  list-style: none;
  padding-left: 4px;
}

.editor-content :deep(.ProseMirror ul[data-type="taskList"] li) {
  display: flex;
  align-items: flex-start;
  gap: 6px;
}

.editor-content :deep(.ProseMirror ul[data-type="taskList"] li > label) {
  margin-top: 3px;
}

.editor-content :deep(.ProseMirror blockquote) {
  border-left: 3px solid var(--border);
  margin: 0 0 8px;
  padding-left: 12px;
  color: var(--text-secondary);
}

.editor-content :deep(.ProseMirror pre) {
  background: var(--bg-hover);
  border-radius: 6px;
  padding: 10px 12px;
  margin: 0 0 8px;
  overflow-x: auto;
  font-family: var(--mono, ui-monospace, monospace);
  font-size: 13px;
}

.editor-content :deep(.ProseMirror code) {
  font-family: var(--mono, ui-monospace, monospace);
  font-size: 0.9em;
}

.editor-content :deep(.ProseMirror pre code) {
  background: none;
  padding: 0;
}

.editor-content :deep(.ProseMirror mark) {
  background: var(--accent-yellow);
  color: inherit;
  border-radius: 2px;
  padding: 0 1px;
}

.editor-content :deep(.ProseMirror img) {
  max-width: 100%;
  border-radius: 6px;
}

.empty-editor {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--text-tertiary);
}

.empty-editor svg {
  width: 48px;
  height: 48px;
}

.empty-editor p {
  font-size: 14px;
  margin: 0;
}
</style>
