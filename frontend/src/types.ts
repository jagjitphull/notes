export interface Folder {
  id: string;
  name: string;
  parentId: string | null;
  color: string | null;
}

export interface Tag {
  id: string;
  name: string;
}

/**
 * A note as listed in the sidebar/list — metadata plus a short preview.
 * The full body lives in the note's Markdown file on disk and is fetched
 * separately (see api.ts's getNoteBody) when the note is opened.
 */
export interface Note {
  id: string;
  title: string;
  plaintextContent: string;
  folderId: string;
  tagIds: string[];
  isPinned: boolean;
  deletedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export type SmartFolderId = "all" | "recently-deleted";
