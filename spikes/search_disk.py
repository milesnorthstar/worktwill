#!/usr/bin/env python3
"""S03 磁盘 WAL 实验，仅在已确认的原生实验根目录内创建合成数据库。"""

import argparse
import hashlib
import json
import random
import sqlite3
import threading
import time
from datetime import datetime, timezone
from pathlib import Path

from search_kernel import QUERIES, normalize, search, setup, short_tokens, summary, verify


def connect(path):
    db = sqlite3.connect(path, timeout=.25)
    db.execute("PRAGMA busy_timeout=250")
    return db


def timed_reads(db, samples, seed):
    schedule = [QUERIES[i % len(QUERIES)] for i in range(samples)]
    random.Random(seed).shuffle(schedule)
    durations = []
    failures = []
    for query in schedule:
        started = time.perf_counter_ns()
        try:
            search(db, query, "short_grams")
        except sqlite3.Error as error:
            failures.append({"query": query.text, "error": str(error)})
        durations.append((time.perf_counter_ns() - started) / 1_000_000)
    return {**summary(durations), "raw_ms": durations, "failures": failures}


def writer(path, stop, ready, observations):
    db = connect(path)
    try:
        originals = list(db.execute("SELECT id,name,location FROM resources WHERE kind='file' AND id>=2000 ORDER BY id LIMIT 1000"))
        sequence = 0
        ready.set()
        while not stop.is_set():
            started = time.perf_counter_ns()
            with db:
                for offset in range(10):
                    rid, base_name, base_location = originals[(sequence * 10 + offset) % len(originals)]
                    old_name, old_location = db.execute("SELECT name,location FROM resources WHERE id=?", (rid,)).fetchone()
                    new_name = f"{base_name}.revision{sequence:08d}"
                    new_location = base_location.rsplit("/", 1)[0] + "/" + new_name
                    # 内容为空的 FTS 表需要携带旧 token 执行删除，随后与业务行同事务更新。
                    db.execute("INSERT INTO tri(tri,rowid,text) VALUES('delete',?,?)", (rid, old_name + "\n" + old_location))
                    db.execute("INSERT INTO grams(grams,rowid,tokens) VALUES('delete',?,?)", (rid, short_tokens(old_name, old_location)))
                    db.execute("UPDATE resources SET name=?,location=? WHERE id=?", (new_name, new_location, rid))
                    db.execute("INSERT INTO tri(rowid,text) VALUES(?,?)", (rid, new_name + "\n" + new_location))
                    db.execute("INSERT INTO grams(rowid,tokens) VALUES(?,?)", (rid, short_tokens(new_name, new_location)))
            sequence += 1
            observations["committed_batches"] = sequence
            observations["row_updates"] = sequence * 10
            observations["commit_ms"].append((time.perf_counter_ns() - started) / 1_000_000)
            wal = Path(str(path) + "-wal")
            observations["peak_observed_wal_bytes"] = max(observations["peak_observed_wal_bytes"], wal.stat().st_size if wal.exists() else 0)
            stop.wait(.01)
    except Exception as error:
        observations["errors"].append(str(error))
        ready.set()
    finally:
        db.close()


def verify_current_projection(db):
    rows = list(db.execute("SELECT id,name,location,kind,allowed FROM resources"))
    memberships = {}
    for rid, workspace in db.execute("SELECT resource_id,workspace_id FROM links"):
        memberships.setdefault(rid, set()).add(workspace)
    for query in QUERIES:
        q = normalize(query.text)
        expected = []
        for rid, name, location, kind, allowed in rows:
            if not allowed or (query.kind != "all" and query.kind != kind):
                continue
            if query.workspace != -1 and query.workspace not in memberships.get(rid, set()):
                continue
            if q not in name and q not in location:
                continue
            rank = 0 if q == name else 1 if name.startswith(q) else 2 if q in name else 3
            expected.append((rank, name, rid, kind))
        expected.sort()
        expected = [(rid, kind, rank) for rank, _, rid, kind in expected[:10]]
        if search(db, query, "short_grams") != expected:
            raise AssertionError(f"projection mismatch after concurrent writes: {query}")
    return {"queries": len(QUERIES), "current_business_rows_match": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--size", type=int, default=100_000)
    parser.add_argument("--samples", type=int, default=570)
    args = parser.parse_args()
    root = args.fixture_root.resolve(strict=True)
    if (root / ".worktwill-fixture-root").read_text() != "worktwill-native-fixture-v1\n":
        parser.error("Not a recognized approved fixture directory")
    if args.size < 10000 or args.samples < 19:
        parser.error("Need >=10000 resources and >=19 samples")
    directory = root / "search-disk"
    directory.mkdir(exist_ok=False)
    database_path = directory / "synthetic.sqlite3"
    print("Building synthetic database", flush=True)
    memory, build = setup(args.size)
    build["correctness"] = verify(memory, args.size)
    db = connect(database_path)
    memory.backup(db)
    memory.close()
    journal = db.execute("PRAGMA journal_mode=WAL").fetchone()[0]
    assert journal == "wal"
    result = {
        "executed_at": datetime.now(timezone.utc).isoformat(),
        "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "kernel_source_sha256": hashlib.sha256(Path(__file__).with_name("search_kernel.py").read_bytes()).hexdigest(),
        "sqlite_version": sqlite3.sqlite_version,
        "database_path": str(database_path), "resources": args.size,
        "scope": "Synthetic on-disk SQLite hot queries and concurrent metadata/FTS writes; not Rust/Tauri, true cold cache, OS scanning or rendered UI.",
        "journal_mode": journal,
        "synchronous": db.execute("PRAGMA synchronous").fetchone()[0],
        "wal_autocheckpoint_pages": db.execute("PRAGMA wal_autocheckpoint").fetchone()[0],
        "build": build,
    }
    print("Measuring without writer", flush=True)
    result["without_writer"] = timed_reads(db, args.samples, 20261119)
    stop, ready = threading.Event(), threading.Event()
    writes = {"committed_batches": 0, "row_updates": 0, "commit_ms": [], "errors": [], "peak_observed_wal_bytes": 0}
    worker = threading.Thread(target=writer, args=(database_path, stop, ready, writes), daemon=True)
    worker.start()
    try:
        if not ready.wait(5):
            raise RuntimeError("Writer did not become ready")
        print("Measuring with concurrent writer", flush=True)
        result["with_writer"] = timed_reads(db, args.samples, 20261119)
    finally:
        stop.set()
        worker.join(timeout=5)
    if worker.is_alive():
        raise RuntimeError("Writer did not stop within bound")
    result["writer"] = writes
    result["post_write_verification"] = verify_current_projection(db)
    result["database_bytes_before_close"] = database_path.stat().st_size
    wal = Path(str(database_path) + "-wal")
    result["wal_bytes_before_close"] = wal.stat().st_size if wal.exists() else 0
    db.close()
    result["database_bytes_after_close"] = database_path.stat().st_size
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({
        "without_writer": {key: value for key, value in result["without_writer"].items() if key != "raw_ms"},
        "with_writer": {key: value for key, value in result["with_writer"].items() if key != "raw_ms"},
        "committed_batches": writes["committed_batches"], "row_updates": writes["row_updates"],
        "writer_errors": writes["errors"], "post_write_verification": result["post_write_verification"],
    }), flush=True)
    if writes["errors"] or not writes["committed_batches"] or result["without_writer"]["failures"] or result["with_writer"]["failures"]:
        raise SystemExit("Experiment recorded failures; inspect result JSON")


if __name__ == "__main__":
    main()
