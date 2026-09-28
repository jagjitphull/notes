import type { Folder, Note, Tag } from "../types";

// Placeholder data for the static Phase 2 layout. Phase 4 replaces this
// module with real reads from the SQLite backend via Tauri commands.

export const folders: Folder[] = [
  { id: "default", name: "Notes", parentId: null },
  { id: "work", name: "Work", parentId: null },
  { id: "personal", name: "Personal", parentId: null },
  { id: "ideas", name: "Ideas", parentId: null },
];

export const tags: Tag[] = [
  { id: "important", name: "Important" },
  { id: "recipes", name: "Recipes" },
  { id: "travel", name: "Travel" },
];

function minutesAgo(minutes: number): string {
  return new Date(Date.now() - minutes * 60_000).toISOString();
}

export const notes: Note[] = [
  {
    id: "n1",
    title: "Q3 roadmap review",
    plaintextContent:
      "Ship the offline sync engine, then revisit the search ranking model.\nNeed sign-off from design before Friday's standup.",
    folderId: "work",
    tagIds: ["important"],
    isPinned: true,
    deletedAt: null,
    updatedAt: minutesAgo(12),
  },
  {
    id: "n2",
    title: "Grocery list",
    plaintextContent:
      "Milk, eggs, sourdough, basil, parmesan.\nDon't forget the good olive oil this time.",
    folderId: "personal",
    tagIds: ["recipes"],
    isPinned: false,
    deletedAt: null,
    updatedAt: minutesAgo(95),
  },
  {
    id: "n3",
    title: "App idea: local-first habit tracker",
    plaintextContent:
      "Single SQLite file, no accounts, export as CSV.\nStreaks rendered as a GitHub-style heatmap.",
    folderId: "ideas",
    tagIds: [],
    isPinned: true,
    deletedAt: null,
    updatedAt: minutesAgo(240),
  },
  {
    id: "n4",
    title: "Portugal trip notes",
    plaintextContent:
      "Book the train from Lisbon to Porto early — sells out.\nLook into the pastel de nata place near Belém tower.",
    folderId: "personal",
    tagIds: ["travel"],
    isPinned: false,
    deletedAt: null,
    updatedAt: minutesAgo(1_380),
  },
  {
    id: "n5",
    title: "1:1 notes — Sam",
    plaintextContent:
      "Discussed the migration timeline and on-call rotation.\nFollow up on the perf review calibration doc.",
    folderId: "work",
    tagIds: [],
    isPinned: false,
    deletedAt: null,
    updatedAt: minutesAgo(2_820),
  },
  {
    id: "n6",
    title: "Sourdough starter log",
    plaintextContent:
      "Day 6: doubled in 4 hours, smells yeasty and sharp.\nFeeding ratio moved to 1:5:5.",
    folderId: "default",
    tagIds: ["recipes"],
    isPinned: false,
    deletedAt: null,
    updatedAt: minutesAgo(4_200),
  },
  {
    id: "n7",
    title: "Old meeting scratchpad",
    plaintextContent:
      "Notes from a planning session that's no longer relevant.\nSafe to remove.",
    folderId: "work",
    tagIds: [],
    isPinned: false,
    deletedAt: minutesAgo(60),
    updatedAt: minutesAgo(60),
  },
];
