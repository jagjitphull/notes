<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { Editor } from "@tiptap/core";
import { EditorContent, useEditor } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Placeholder from "@tiptap/extension-placeholder";
import Image from "@tiptap/extension-image";
import HighlightBase from "@tiptap/extension-highlight";
import { Markdown } from "tiptap-markdown";
import markdownItMark from "markdown-it-mark";
import Icon from "./icons/Icon.vue";
import type { IconName } from "./icons/icons";
import type { Folder, Note, Tag } from "../types";
import {
  clearHighlights,
  findMatches,
  FindReplace,
  highlightMatches,
  replaceAllMatches,
  replaceMatch,
  scrollToMatch,
  type FindMatch,
} from "../tiptap/findReplace";
import { exitSuggestion } from "@tiptap/suggestion";
import { extractLinkedTitles, NoteLink } from "../tiptap/noteLink";

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
  notes: Note[];
  folders: Folder[];
  tags: Tag[];
}>();

const emit = defineEmits<{
  togglePin: [];
  toggleDeleted: [];
  addTag: [];
  removeTag: [name: string];
  navigateToNote: [id: string];
}>();

// Other notes this one can link to via [[Title]] - excludes the current
// note (linking to itself isn't useful), trashed notes (nothing to
// navigate to), and templates (not "real" notes to link between). Keyed
// by lowercased title since links resolve by title, not id: renaming a
// note breaks links elsewhere that pointed at its old title, rather than
// tracking renames - see tiptap/noteLink.ts.
const linkTargets = computed(() =>
  props.notes.filter((n) => !n.deletedAt && !n.isTemplate && n.id !== props.note?.id),
);

const noteTitleIndex = computed(() => {
  const map = new Map<string, string>();
  for (const n of linkTargets.value) {
    const key = n.title.trim().toLowerCase();
    if (key && !map.has(key)) map.set(key, n.id);
  }
  return map;
});

const suggestionTitles = computed(() => {
  const seen = new Set<string>();
  const titles: string[] = [];
  for (const n of linkTargets.value) {
    const trimmed = n.title.trim();
    const key = trimmed.toLowerCase();
    if (!key || seen.has(key)) continue;
    seen.add(key);
    titles.push(trimmed);
  }
  return titles;
});

function handleNoteLinkNavigate(title: string) {
  const id = noteTitleIndex.value.get(title.trim().toLowerCase());
  if (id) emit("navigateToNote", id);
}

// "Linked Mentions": the reverse of the forward links above - other notes
// whose saved body contains a [[This Note's Title]] link. Computed from
// plaintextContent (already loaded for every note in the list, full body
// past the title line - see note_file::extract_preview on the Rust side)
// rather than a fetch per candidate note.
const backlinks = computed(() => {
  const title = props.note?.title.trim().toLowerCase();
  if (!title) return [];
  return linkTargets.value
    .filter((n) =>
      extractLinkedTitles(n.plaintextContent).some((t) => t.toLowerCase() === title),
    )
    .map((n) => ({ id: n.id, title: n.title.trim() || t("common.newNote") }));
});

const noteTags = computed(() =>
  props.tags
    .filter((t) => props.note?.tagIds.includes(t.id))
    .slice()
    .sort((a, b) => a.name.localeCompare(b.name)),
);

const body = defineModel<string>("body", { default: "" });

const { t } = useI18n();

const isDeleted = computed(() => !!props.note?.deletedAt);

const editor = useEditor({
  content: "",
  editable: !isDeleted.value,
  extensions: [
    StarterKit.configure({ heading: { levels: [1, 2, 3] } }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Placeholder.configure({ placeholder: t("editor.placeholder") }),
    // Pasted/dropped images are inserted as data: URIs (see handlePaste/
    // handleDrop below) - @tiptap/extension-image defaults to rejecting
    // those specifically (allowBase64: false), which doesn't break the
    // insert itself but silently drops the image the next time the note
    // is re-parsed from its saved markdown (reopening it, or even just
    // switching to another note and back).
    Image.configure({ allowBase64: true }),
    Highlight,
    FindReplace,
    NoteLink.configure({
      getNoteTitles: () => suggestionTitles.value,
      isResolved: (title) => noteTitleIndex.value.has(title.trim().toLowerCase()),
      onNavigate: handleNoteLinkNavigate,
    }),
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
    attributes: {
      role: "textbox",
      "aria-multiline": "true",
      "aria-label": t("editor.contentLabel"),
    },
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
    // A screenshot tool copies image bytes straight to the clipboard
    // (no file to drag), so pasting needs its own handler - handleDrop
    // above only ever sees files from a real drag-and-drop.
    handlePaste(view, event) {
      // .files first, matching handleDrop above - some WebKit builds
      // don't populate DataTransferItem.kind/getAsFile() reliably for a
      // clipboard image, but do populate .files.
      let files = Array.from(event.clipboardData?.files ?? []).filter((f) =>
        f.type.startsWith("image/"),
      );
      if (files.length === 0) {
        files = Array.from(event.clipboardData?.items ?? [])
          .filter((item) => item.kind === "file" && item.type.startsWith("image/"))
          .map((item) => item.getAsFile())
          .filter((f): f is File => f !== null);
      }
      if (files.length === 0) return false;
      event.preventDefault();

      const pos = view.state.selection.from;
      for (const file of files) {
        const reader = new FileReader();
        reader.onload = () => {
          const node = view.state.schema.nodes.image.create({ src: reader.result });
          const tr = view.state.tr.insert(pos, node);
          view.dispatch(tr);
        };
        reader.readAsDataURL(file);
      }
      return true;
    },
  },
  onUpdate({ editor }) {
    body.value = editor.storage.markdown.getMarkdown();
    convertBlobImages(editor);
    // Keeps hit-highlighting and the match count correct through edits
    // made while the find bar is open (typing in the note, or a
    // replace/replace-all this same component just performed).
    if (findOpen.value) refreshMatches();
  },
});

// Safety net for pasted images: on at least one WebKitGTK build, a
// pasted (not dragged) image doesn't reach handlePaste's clipboardData
// at all - something upstream of it inserts an <img src="blob:...">
// directly via the platform's own default paste handling. A blob: URL
// only resolves for this page session, so it silently breaks (shows a
// broken image) the next time the note is opened. This normalizes any
// such image, however it got inserted, into a persistent data: URL.
const blobConversionsInFlight = new Set<string>();

function convertBlobImages(editor: Editor) {
  editor.state.doc.descendants((node) => {
    const src = node.attrs.src as string | undefined;
    if (node.type.name !== "image" || !src?.startsWith("blob:")) return;
    if (blobConversionsInFlight.has(src)) return;
    blobConversionsInFlight.add(src);

    fetch(src)
      .then((res) => res.blob())
      .then(
        (blob) =>
          new Promise<string>((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result as string);
            reader.onerror = () => reject(reader.error);
            reader.readAsDataURL(blob);
          }),
      )
      .then((dataUrl) => {
        if (editor.isDestroyed) return;
        editor.state.doc.descendants((n, pos) => {
          if (n.type.name === "image" && n.attrs.src === src) {
            editor.view.dispatch(editor.view.state.tr.setNodeAttribute(pos, "src", dataUrl));
            return false;
          }
        });
      })
      .catch(() => {
        // Best-effort: if the blob URL is already gone, there's nothing
        // left to recover it from.
      });
  });
}

// Sync content into the editor whenever `body` diverges from what the
// editor itself last produced — i.e. it changed for an external reason
// (a different note was selected, or its body was freshly fetched). This
// naturally no-ops on the editor's own onUpdate-triggered writes, and is
// immune to load-order races since it just compares current values.
watch(body, (value) => {
  if (!editor.value) return;
  if (value !== editor.value.storage.markdown.getMarkdown()) {
    editor.value.commands.setContent(value, { emitUpdate: false });
    // A note-linking autocomplete popup left open (e.g. the user switched
    // notes via the sidebar instead of dismissing it) would otherwise keep
    // floating over content it no longer applies to.
    exitSuggestion(editor.value.view);
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

const findOpen = ref(false);
const findQuery = ref("");
const replaceQuery = ref("");
const caseSensitive = ref(false);
const matches = ref<FindMatch[]>([]);
const currentMatchIndex = ref(-1);
const findInputRef = ref<HTMLInputElement | null>(null);

function scrollToCurrentMatch() {
  const e = editor.value;
  const match = matches.value[currentMatchIndex.value];
  if (e && match) scrollToMatch(e, match);
}

// Re-runs the search from scratch: the only path that's correct after the
// document itself changed (typing, a replace this component just made).
// Navigation (next/prev) doesn't need this - see setCurrentMatch below.
function refreshMatches(desiredIndex = currentMatchIndex.value) {
  const e = editor.value;
  if (!e) return;
  matches.value = findMatches(e, findQuery.value, caseSensitive.value);
  currentMatchIndex.value =
    matches.value.length === 0 ? -1 : Math.min(Math.max(desiredIndex, 0), matches.value.length - 1);
  highlightMatches(e, matches.value, currentMatchIndex.value);
  scrollToCurrentMatch();
}

function setCurrentMatch(index: number) {
  const e = editor.value;
  if (!e || matches.value.length === 0) return;
  currentMatchIndex.value = ((index % matches.value.length) + matches.value.length) % matches.value.length;
  highlightMatches(e, matches.value, currentMatchIndex.value);
  scrollToCurrentMatch();
}

function goToNextMatch() {
  setCurrentMatch(currentMatchIndex.value + 1);
}

function goToPreviousMatch() {
  setCurrentMatch(currentMatchIndex.value - 1);
}

async function openFind() {
  findOpen.value = true;
  refreshMatches(0);
  await nextTick();
  findInputRef.value?.focus();
  findInputRef.value?.select();
}

function closeFind({ refocus = true } = {}) {
  findOpen.value = false;
  matches.value = [];
  currentMatchIndex.value = -1;
  if (editor.value) clearHighlights(editor.value);
  if (refocus) editor.value?.commands.focus();
}

function replaceCurrent() {
  const e = editor.value;
  const match = matches.value[currentMatchIndex.value];
  if (!e || !match || isDeleted.value) return;
  replaceMatch(e, match, replaceQuery.value);
  // onUpdate refreshes matches/highlights for us (findOpen is true).
}

function replaceAll() {
  const e = editor.value;
  if (!e || matches.value.length === 0 || isDeleted.value) return;
  replaceAllMatches(e, matches.value, replaceQuery.value);
}

watch([findQuery, caseSensitive], () => {
  if (findOpen.value) refreshMatches(0);
});

// Switching notes leaves findQuery/caseSensitive as-is (reopening Find on
// the new note keeps the last search term, like a browser's find bar) but
// drops the match list and highlights, which point at the old note's text.
watch(
  () => props.note?.id,
  () => {
    if (findOpen.value) closeFind({ refocus: false });
  },
);

const matchCountLabel = computed(() => {
  if (!findQuery.value) return "";
  if (matches.value.length === 0) return t("editor.find.noResults");
  return t("editor.find.matchCount", {
    current: currentMatchIndex.value + 1,
    total: matches.value.length,
  });
});

defineExpose({ focusEditor, openFind });

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
  icon: IconName;
  isActive: () => boolean;
  run: () => void;
};

const toolbarActions = computed<ToolbarAction[]>(() => {
  const e = editor.value;
  if (!e) return [];
  const chain = () => e.chain().focus();
  return [
    { label: t("editor.toolbar.bold"), icon: "bold", isActive: () => e.isActive("bold"), run: () => chain().toggleBold().run() },
    { label: t("editor.toolbar.italic"), icon: "italic", isActive: () => e.isActive("italic"), run: () => chain().toggleItalic().run() },
    { label: t("editor.toolbar.underline"), icon: "underline", isActive: () => e.isActive("underline"), run: () => chain().toggleUnderline().run() },
    { label: t("editor.toolbar.strikethrough"), icon: "strikethrough", isActive: () => e.isActive("strike"), run: () => chain().toggleStrike().run() },
    { label: t("editor.toolbar.highlight"), icon: "highlight", isActive: () => e.isActive("highlight"), run: () => chain().toggleHighlight().run() },
    { label: t("editor.toolbar.heading1"), icon: "heading1", isActive: () => e.isActive("heading", { level: 1 }), run: () => chain().toggleHeading({ level: 1 }).run() },
    { label: t("editor.toolbar.heading2"), icon: "heading2", isActive: () => e.isActive("heading", { level: 2 }), run: () => chain().toggleHeading({ level: 2 }).run() },
    { label: t("editor.toolbar.checklist"), icon: "checklist", isActive: () => e.isActive("taskList"), run: () => chain().toggleTaskList().run() },
    { label: t("editor.toolbar.bulletList"), icon: "bulletList", isActive: () => e.isActive("bulletList"), run: () => chain().toggleBulletList().run() },
    { label: t("editor.toolbar.numberedList"), icon: "numberedList", isActive: () => e.isActive("orderedList"), run: () => chain().toggleOrderedList().run() },
    { label: t("editor.toolbar.codeBlock"), icon: "code", isActive: () => e.isActive("codeBlock"), run: () => chain().toggleCodeBlock().run() },
    { label: t("editor.toolbar.quote"), icon: "quote", isActive: () => e.isActive("blockquote"), run: () => chain().toggleBlockquote().run() },
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
            :aria-label="action.label"
            :aria-pressed="action.isActive()"
            :disabled="isDeleted"
            @click="action.run"
          >
            <Icon :name="action.icon" />
          </button>
        </div>
        <div class="toolbar-right">
          <span class="editor-meta">{{ formattedDate }} &middot; {{ folderName }}</span>
          <button
            class="icon-button"
            :class="{ active: findOpen }"
            :title="t('editor.find.openFind')"
            :aria-label="t('editor.find.openFind')"
            :aria-pressed="findOpen"
            @click="findOpen ? closeFind() : openFind()"
          >
            <Icon name="search" />
          </button>
          <button
            class="icon-button"
            :class="{ active: note.isPinned }"
            :title="note.isPinned ? t('common.unpin') : t('common.pin')"
            :aria-label="note.isPinned ? t('common.unpin') : t('common.pin')"
            :aria-pressed="note.isPinned"
            :disabled="isDeleted"
            @click="emit('togglePin')"
          >
            <Icon name="pin" />
          </button>
          <button
            class="icon-button"
            :title="isDeleted ? t('common.restore') : t('common.delete')"
            :aria-label="isDeleted ? t('common.restore') : t('common.delete')"
            @click="emit('toggleDeleted')"
          >
            <Icon :name="isDeleted ? 'restore' : 'trash'" />
          </button>
        </div>
      </div>

      <div v-if="findOpen" class="find-bar" role="search" :aria-label="t('editor.find.openFind')">
        <input
          ref="findInputRef"
          v-model="findQuery"
          type="text"
          class="find-input"
          :placeholder="t('editor.find.findPlaceholder')"
          @keydown.enter.exact.prevent="goToNextMatch"
          @keydown.enter.shift.exact.prevent="goToPreviousMatch"
          @keydown.escape.prevent="closeFind()"
        />
        <span class="find-count">{{ matchCountLabel }}</span>
        <button
          class="icon-button"
          :disabled="matches.length === 0"
          :title="t('editor.find.previous')"
          :aria-label="t('editor.find.previous')"
          @click="goToPreviousMatch"
        >
          <Icon name="chevronUp" />
        </button>
        <button
          class="icon-button"
          :disabled="matches.length === 0"
          :title="t('editor.find.next')"
          :aria-label="t('editor.find.next')"
          @click="goToNextMatch"
        >
          <Icon name="chevronDown" />
        </button>
        <button
          class="find-case-toggle"
          :class="{ active: caseSensitive }"
          type="button"
          :title="t('editor.find.caseSensitive')"
          :aria-label="t('editor.find.caseSensitive')"
          :aria-pressed="caseSensitive"
          @click="caseSensitive = !caseSensitive"
        >
          Aa
        </button>
        <input
          v-model="replaceQuery"
          type="text"
          class="find-input"
          :disabled="isDeleted"
          :placeholder="t('editor.find.replacePlaceholder')"
          @keydown.enter.exact.prevent="replaceCurrent"
          @keydown.escape.prevent="closeFind()"
        />
        <button
          class="find-text-button"
          :disabled="matches.length === 0 || isDeleted"
          @click="replaceCurrent"
        >
          {{ t('editor.find.replace') }}
        </button>
        <button
          class="find-text-button"
          :disabled="matches.length === 0 || isDeleted"
          @click="replaceAll"
        >
          {{ t('editor.find.replaceAll') }}
        </button>
        <button
          class="icon-button"
          :title="t('common.close')"
          :aria-label="t('common.close')"
          @click="closeFind()"
        >
          <Icon name="close" />
        </button>
      </div>

      <div v-if="!isDeleted" class="tags-row">
        <span
          v-for="tag in noteTags"
          :key="tag.id"
          class="tag-chip"
          :style="tag.color ? { color: tag.color } : undefined"
        >
          #{{ tag.name }}
          <button
            class="tag-remove"
            :title="t('editor.removeTag')"
            :aria-label="t('editor.removeTagLabel', { name: tag.name })"
            @click="emit('removeTag', tag.name)"
          >
            &times;
          </button>
        </span>
        <button class="add-tag-button" :title="t('editor.addTag')" @click="emit('addTag')">
          {{ t('editor.addTagButton') }}
        </button>
      </div>

      <div class="editor-canvas">
        <EditorContent :editor="editor" class="editor-content" />

        <div v-if="backlinks.length > 0" class="backlinks">
          <h3 class="backlinks-title">{{ t('editor.backlinks.title') }}</h3>
          <ul class="backlinks-list">
            <li v-for="link in backlinks" :key="link.id">
              <button class="backlink-item" @click="emit('navigateToNote', link.id)">
                <Icon name="document" />
                <span>{{ link.title }}</span>
              </button>
            </li>
          </ul>
        </div>
      </div>
    </template>

    <div v-else class="empty-editor">
      <Icon name="document" />
      <p>{{ t('editor.emptyState') }}</p>
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
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 26px;
  height: 26px;
  padding: 0 5px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
}

.format-button svg {
  width: 16px;
  height: 16px;
}

.format-button:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.format-button.active {
  background: var(--bg-selected);
  color: var(--accent-blue-text);
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

.find-bar {
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-sidebar);
}

.find-input {
  flex: 1 1 120px;
  min-width: 90px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-editor);
  color: var(--text-primary);
  font: inherit;
  font-size: 13px;
  padding: 4px 8px;
}

.find-input:disabled {
  opacity: 0.5;
}

.find-input:focus-visible {
  outline: 2px solid var(--accent-blue);
  outline-offset: -1px;
}

.find-count {
  flex: 0 0 auto;
  font-size: 12px;
  color: var(--text-tertiary);
  min-width: 56px;
  text-align: center;
}

.find-case-toggle {
  flex: 0 0 auto;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.find-case-toggle:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.find-case-toggle.active {
  background: var(--bg-selected);
  color: var(--accent-blue-text);
}

.find-text-button {
  flex: 0 0 auto;
  padding: 4px 10px;
  border: none;
  border-radius: 6px;
  background: var(--bg-hover);
  color: var(--text-primary);
  font-size: 12.5px;
  cursor: pointer;
  white-space: nowrap;
}

.find-text-button:hover:not(:disabled) {
  background: var(--bg-selected);
}

.find-text-button:disabled {
  opacity: 0.4;
  cursor: default;
}

.tags-row {
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 8px 48px 0;
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
  color: var(--accent-blue-text);
}

.editor-canvas {
  flex: 1 1 auto;
  /* Flex items default to min-height: auto (content-based), which
     overrides flex-shrink and defeats overflow-y: auto - a long note
     would just grow the box instead of scrolling within it. */
  min-height: 0;
  overflow-y: auto;
  /* No max-width/centering: a capped column (even a generous one) still
     reads as wasted space once the pane is wide, per direct feedback.
     Content now spans the same width as the toolbar above it - just this
     padding on either side, at any pane width. */
  padding: 16px 48px 48px;
  width: 100%;
  box-sizing: border-box;
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
  /* Not `inherit`: in dark mode that puts near-white body text on a
     similarly light yellow highlight (~1.2:1 contrast, unreadable).
     The highlight background needs dark text in both themes. */
  color: #1c1c1e;
  border-radius: 2px;
  padding: 0 1px;
}

/* Find results: pale yellow for every hit, a stronger orange ring on
   whichever one is "current" - same convention browsers use for their
   own in-page find, and visually distinct from the ==highlight== mark
   above despite the shared yellow family. */
.editor-content :deep(.find-match) {
  background: rgba(242, 183, 5, 0.35);
  border-radius: 2px;
}

.editor-content :deep(.find-match-current) {
  background: rgba(255, 149, 0, 0.55);
  box-shadow: 0 0 0 1px rgba(255, 149, 0, 0.9);
  border-radius: 2px;
}

.editor-content :deep(.ProseMirror img) {
  max-width: 100%;
  border-radius: 6px;
}

.backlinks {
  margin-top: 32px;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}

.backlinks-title {
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.02em;
  color: var(--text-tertiary);
  margin: 0 0 4px;
}

.backlinks-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.backlink-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  text-align: left;
  padding: 6px 8px;
  margin: 0 -8px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-primary);
  font-size: 13.5px;
  cursor: pointer;
}

.backlink-item:hover {
  background: var(--bg-hover);
}

.backlink-item svg {
  width: 14px;
  height: 14px;
  color: var(--text-tertiary);
  flex: 0 0 auto;
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
