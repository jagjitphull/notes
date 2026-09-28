export interface Folder {
  id: string;
  name: string;
  parentId: string | null;
}

export interface Tag {
  id: string;
  name: string;
}

export interface Note {
  id: string;
  title: string;
  plaintextContent: string;
  folderId: string;
  tagIds: string[];
  isPinned: boolean;
  deletedAt: string | null;
  updatedAt: string;
}

export type SmartFolderId = "all" | "recently-deleted";
