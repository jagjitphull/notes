-- Optional per-folder accent color (a CSS color string, e.g. "#0a84ff"),
-- shown as a tint on the folder's icon. NULL means "use the default color".
-- Not touched by full_rescan's folder upsert (see store::full_rescan),
-- so it survives a rescan even though folders themselves are otherwise
-- fully rebuilt from the directory tree on every scan.
ALTER TABLE folders ADD COLUMN color TEXT;
