-- Optional per-tag accent color (a CSS color string, e.g. "#0a84ff"), shown
-- as a tint on the tag's "#" icon and its chips in the editor. NULL means
-- "use the default color". Not touched by ensure_tag's find-or-create (see
-- store::ensure_tag), which only INSERTs id+name for a brand-new tag and
-- otherwise just looks up the existing row - so an existing tag's color
-- survives every call, the same way folder colors survive full_rescan.
ALTER TABLE tags ADD COLUMN color TEXT;
