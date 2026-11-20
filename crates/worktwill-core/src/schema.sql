CREATE TABLE settings(key TEXT PRIMARY KEY, value INTEGER NOT NULL);
INSERT INTO settings VALUES('permission_version',0),('revision',0);
CREATE TABLE scan_roots(
  id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE, active INTEGER NOT NULL,
  paused INTEGER NOT NULL DEFAULT 0, generation INTEGER NOT NULL DEFAULT 0,
  scan_state TEXT NOT NULL DEFAULT 'idle', scanned INTEGER NOT NULL DEFAULT 0,
  failures INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE scan_exclusions(
  root_id TEXT NOT NULL REFERENCES scan_roots(id), relative_path TEXT NOT NULL,
  PRIMARY KEY(root_id,relative_path)
);
CREATE TABLE resources(
  id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK(kind IN ('file','web')),
  name TEXT NOT NULL, location TEXT NOT NULL, identity TEXT,
  modified_at REAL NOT NULL DEFAULT 0, size REAL NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'available', favorite INTEGER NOT NULL DEFAULT 0,
  created_at REAL NOT NULL
);
CREATE UNIQUE INDEX active_file_identity ON resources(identity) WHERE identity IS NOT NULL;
CREATE TABLE file_roots(
  root_id TEXT NOT NULL REFERENCES scan_roots(id), resource_id TEXT NOT NULL REFERENCES resources(id),
  generation INTEGER NOT NULL, PRIMARY KEY(root_id,resource_id)
);
CREATE INDEX root_resources ON file_roots(resource_id,root_id);
CREATE TABLE workspaces(id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at REAL NOT NULL);
CREATE TABLE workspace_resources(
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  resource_id TEXT NOT NULL REFERENCES resources(id), added_at REAL NOT NULL,
  PRIMARY KEY(workspace_id,resource_id)
);
CREATE TABLE activities(
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL,
  resource_id TEXT REFERENCES resources(id), at REAL NOT NULL
);
CREATE TABLE operations(id TEXT PRIMARY KEY, payload TEXT NOT NULL, result TEXT NOT NULL);
CREATE TABLE search_documents(
  rowid INTEGER PRIMARY KEY, entity_id TEXT NOT NULL UNIQUE, kind TEXT NOT NULL,
  name TEXT NOT NULL, location TEXT NOT NULL
);
CREATE INDEX search_documents_name ON search_documents(name,entity_id);
CREATE VIRTUAL TABLE search_tri USING fts5(text, tokenize='trigram');
CREATE VIRTUAL TABLE search_short USING fts5(tokens, detail=none);
PRAGMA user_version=1;
