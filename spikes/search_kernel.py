#!/usr/bin/env python3
"""S03 查询内核实验：只在内存生成合成数据，不读取或扫描用户目录。"""

import argparse
import hashlib
import json
import math
import os
import platform
import random
import resource
import sqlite3
import statistics
import sys
import time
import unicodedata
from dataclasses import asdict, dataclass
from datetime import datetime, timezone
from pathlib import Path


@dataclass(frozen=True)
class Query:
    text: str
    kind: str = "all"
    workspace: int = -1


def normalize(value):
    return unicodedata.normalize("NFC", value).casefold()


def gram_token(value):
    return "g" + str(len(value)) + "x" + "x".join(f"{ord(c):x}" for c in value)


def short_tokens(name, location):
    # 分字段取 gram，避免把名称末尾和路径开头拼成虚假的连续命中。
    tokens = set()
    for value in (name, location):
        for width in (1, 2):
            tokens.update(gram_token(value[i : i + width]) for i in range(len(value) - width + 1))
    return " ".join(sorted(tokens))


def fixtures(count):
    controls = [
        ("合作合同_张三.docx", "/fixtures/客户A/合作合同_张三.docx", "file", 1),
        ("合作合同_张三.docx", "/fixtures/未授权/合作合同_张三.docx", "file", 0),
        ("张", "/fixtures/桌面/张", "file", 1),
        ("报价_100%_确认.xlsx", "/fixtures/项目 A/报价_100%_确认.xlsx", "file", 1),
        ('带"引号"的合同.docx', '/fixtures/项目 A/带"引号"的合同.docx', "file", 1),
        ("合同", "/fixtures/桌面/合同", "file", 1),
        ("合同摘要.pdf", "/fixtures/下载/合同摘要.pdf", "file", 1),
        ("旧合同.pdf", "/fixtures/下载/旧合同.pdf", "file", 1),
        ("Cafe\u0301方案.pdf", "/fixtures/文档/Cafe\u0301方案.pdf", "file", 1),
        ("literal OR query.txt", "/fixtures/文档/literal OR query.txt", "file", 1),
        ("招聘后台", "https://example.test/jobs?q=2026&team=HR", "web", 1),
        ("2026招聘", "", "workspace", 1),
    ]
    people = ("张三", "李四", "王五", "陈六", "Alex", "Élodie", "赵七")
    templates = (
        "合作合同_{person}_{number}.docx",
        "招聘计划_{person}_{number}.xlsx",
        "活动方案_{person}_{number}.pptx",
        "预算复盘_{person}_{number}.pdf",
        "Interview notes {person} {number}.docx",
        "客户资料_{person}_{number}.zip",
        "设计稿_{person}_{number}.png",
        "入职清单_{person}_{number}.xlsx",
    )
    for index in range(count):
        if index < len(controls):
            name, location, kind, allowed = controls[index]
        else:
            person = people[index % len(people)]
            number = f"{index:06d}"
            if index % 37 == 0:
                kind, name, location = "workspace", f"项目{person}_{number}", ""
            elif index % 17 == 0:
                kind = "web"
                name = f"招聘后台_{person}_{number}"
                location = f"https://example.test/2026招聘/{number}?owner={person}&v=2"
            else:
                kind = "file"
                name = templates[index % len(templates)].format(person=person, number=number)
                location = f"/fixtures/文档/客户{index % 41}/2026/项目 A/{name}"
            allowed = int(index % 11 != 0)
        yield (index + 1, normalize(name), normalize(location), kind, allowed)


def links_for(resource_id):
    groups = set()
    if resource_id % 3 == 0:
        groups.add(resource_id % 19)
    if resource_id % 13 == 0:
        groups.add((resource_id + 3) % 19)
    return groups


def allocated_bytes(db):
    return db.execute("PRAGMA page_count").fetchone()[0] * db.execute("PRAGMA page_size").fetchone()[0]


def setup(count):
    started = time.perf_counter()
    db = sqlite3.connect(":memory:")
    db.execute("PRAGMA temp_store=MEMORY")
    db.executescript("""
        CREATE TABLE resources(
            id INTEGER PRIMARY KEY, name TEXT NOT NULL, location TEXT NOT NULL,
            kind TEXT NOT NULL, allowed INTEGER NOT NULL
        );
        CREATE TABLE links(
            resource_id INTEGER NOT NULL, workspace_id INTEGER NOT NULL,
            PRIMARY KEY(resource_id, workspace_id)
        ) WITHOUT ROWID;
        CREATE VIRTUAL TABLE tri USING fts5(text, content='', tokenize='trigram');
        CREATE VIRTUAL TABLE grams USING fts5(tokens, content='', detail=none);
    """)
    with db:
        db.executemany("INSERT INTO resources VALUES(?,?,?,?,?)", fixtures(count))
        db.executemany(
            "INSERT INTO links VALUES(?,?)",
            ((rid, group) for rid in range(1, count + 1) for group in links_for(rid)),
        )
    base_bytes = allocated_bytes(db)
    base_seconds = time.perf_counter() - started
    tri_started = time.perf_counter()
    with db:
        db.executemany(
            "INSERT INTO tri(rowid,text) VALUES(?,?)",
            ((rid, name + "\n" + location) for rid, name, location, _, _ in fixtures(count)),
        )
        db.execute("INSERT INTO tri(tri) VALUES('optimize')")
    tri_bytes = allocated_bytes(db)
    tri_seconds = time.perf_counter() - tri_started
    grams_started = time.perf_counter()
    with db:
        db.executemany(
            "INSERT INTO grams(rowid,tokens) VALUES(?,?)",
            ((rid, short_tokens(name, location)) for rid, name, location, _, _ in fixtures(count)),
        )
        db.execute("INSERT INTO grams(grams) VALUES('optimize')")
    info = {
        "resources": count,
        "base_build_seconds": base_seconds,
        "trigram_build_seconds": tri_seconds,
        "short_gram_build_seconds": time.perf_counter() - grams_started,
        "allocated_sqlite_bytes": {
            "base": base_bytes,
            "after_trigram": tri_bytes,
            "after_short_grams": allocated_bytes(db),
        },
        "storage_note": "In-memory SQLite allocated pages, includes free pages; not disk/WAL measurements.",
    }
    return db, info


def statement(query, strategy):
    value = normalize(query.text)
    if not value:
        raise ValueError("Empty query is not part of this experiment")
    if len(value) >= 3:
        join = "JOIN tri ON tri.rowid=r.id"
        candidate = "tri MATCH :match AND"
        match = '"' + value.replace('"', '""') + '"'
    elif strategy == "short_grams":
        join = "JOIN grams ON grams.rowid=r.id"
        candidate, match = "grams MATCH :match AND", gram_token(value)
    else:
        join, candidate, match = "", "", ""
    sql = f"""
        SELECT r.id, r.kind,
          CASE WHEN r.name=:q THEN 0
               WHEN substr(r.name,1,length(:q))=:q THEN 1
               WHEN instr(r.name,:q)>0 THEN 2 ELSE 3 END AS match_rank
        FROM resources r {join}
        WHERE {candidate} r.allowed=1
          AND (:kind='all' OR r.kind=:kind)
          AND (:workspace=-1 OR EXISTS(
            SELECT 1 FROM links l WHERE l.resource_id=r.id AND l.workspace_id=:workspace
          ))
          AND (instr(r.name,:q)>0 OR instr(r.location,:q)>0)
        ORDER BY match_rank, r.name, r.id LIMIT 10
    """
    return sql, {"q": value, "match": match, "kind": query.kind, "workspace": query.workspace}


def search(db, query, strategy):
    sql, params = statement(query, strategy)
    return list(db.execute(sql, params))


def oracle(count, queries):
    # 完整遍历仅用于实验正确性参照，不进入候选热查询路径。
    answers = {}
    for query in queries:
        q = normalize(query.text)
        matches = []
        for rid, name, location, kind, allowed in fixtures(count):
            if not allowed or (query.kind != "all" and kind != query.kind):
                continue
            if query.workspace != -1 and query.workspace not in links_for(rid):
                continue
            if q not in name and q not in location:
                continue
            rank = 0 if name == q else 1 if name.startswith(q) else 2 if q in name else 3
            matches.append((rank, name, rid, kind))
        matches.sort()
        answers[query] = [(rid, kind, rank) for rank, _, rid, kind in matches[:10]]
    return answers


def percentile(values, p):
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * p) - 1)]


def summary(values):
    return {
        "samples": len(values), "p50_ms": statistics.median(values),
        "p95_ms": percentile(values, .95), "p99_ms": percentile(values, .99),
        "max_ms": max(values),
    }


QUERIES = [
    Query("张"), Query("合", "file"), Query("张三"), Query("合同"),
    Query("2026招聘"), Query("Interview"), Query("客户7", "file"),
    Query("招聘", "web"), Query("项目", "workspace"), Query("张三", "all", 7),
    Query("合作合同_张三.docx"), Query("100%"), Query('"引号"'),
    Query("Café"), Query("literal OR"), Query("q=2026&team=HR"),
    Query("从不存在的文件"), Query("项目 A"), Query("_"),
]


def verify(db, count):
    answers = oracle(count, QUERIES)
    for strategy in ("scan_short", "short_grams"):
        for query, expected in answers.items():
            actual = search(db, query, strategy)
            if actual != expected:
                raise AssertionError({"strategy": strategy, "query": asdict(query), "actual": actual, "expected": expected})
        assert search(db, Query("合同"), strategy)[0][0] == 6, "Exact name must rank first"
        assert all(row[0] != 2 for row in search(db, Query("合作合同_张三.docx"), strategy))
        # 故意把所有可命中资源撤权，旧索引不得继续返回可见记录。
        db.execute("SAVEPOINT permission_probe")
        db.execute("UPDATE resources SET allowed=0")
        assert search(db, Query("张"), strategy) == []
        db.execute("ROLLBACK TO permission_probe")
        db.execute("RELEASE permission_probe")
    return {"queries_per_strategy": len(QUERIES), "oracle_match": True, "permission_filter": True}


def benchmark(db, samples, seed):
    rng = random.Random(seed)
    schedule = [QUERIES[i % len(QUERIES)] for i in range(samples)]
    rng.shuffle(schedule)
    data = {strategy: {query: [] for query in QUERIES} for strategy in ("scan_short", "short_grams")}
    for query in QUERIES:
        for strategy in data:
            search(db, query, strategy)
    # 交错两种策略以减少固定先后次序造成的缓存与温度偏差。
    for index, query in enumerate(schedule):
        strategies = list(data) if index % 2 else list(reversed(data))
        for strategy in strategies:
            started = time.perf_counter_ns()
            search(db, query, strategy)
            data[strategy][query].append((time.perf_counter_ns() - started) / 1_000_000)
    result = {}
    for strategy, per_query in data.items():
        flattened = [value for values in per_query.values() for value in values]
        plans = {}
        for value in ("张", "合同", "2026招聘"):
            sql, params = statement(Query(value), strategy)
            plans[value] = [list(row) for row in db.execute("EXPLAIN QUERY PLAN " + sql, params)]
        result[strategy] = {
            "overall": summary(flattened),
            "per_query": [
                {"query": asdict(query), **summary(values), "raw_ms": values}
                for query, values in per_query.items() if values
            ],
            "query_plans": plans,
        }
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sizes", nargs="+", type=int, default=[10_000, 50_000, 100_000, 500_000])
    parser.add_argument("--samples", type=int, default=570)
    parser.add_argument("--seed", type=int, default=20261119)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if min(args.sizes) < 20 or args.samples < len(QUERIES):
        parser.error("Need at least 20 rows and one sample per query")
    result = {
        "started_at": datetime.now(timezone.utc).isoformat(),
        "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "environment": {"os": platform.platform(), "machine": platform.machine(), "cpu_count": os.cpu_count(),
                        "python": sys.version, "sqlite": sqlite3.sqlite_version},
        "scope": "Synthetic in-memory SQL kernel; not Rust/Tauri, disk, WAL, concurrent writes, or rendered UI acceptance.",
        "fixture_seed": args.seed,
        "query_count": len(QUERIES),
        "normalization": "NFC + casefold; candidate for this experiment only",
        "runs": [],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for count in args.sizes:
        print(json.dumps({"phase": "building", "resources": count}), flush=True)
        db, info = setup(count)
        print(json.dumps({"phase": "verifying", "resources": count}), flush=True)
        info["correctness"] = verify(db, count)
        info["strategies"] = benchmark(db, args.samples, args.seed + count)
        info["peak_rss_bytes"] = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * (1 if sys.platform == "darwin" else 1024)
        result["runs"].append(info)
        result["updated_at"] = datetime.now(timezone.utc).isoformat()
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps({"phase": "completed", "resources": count,
                          "strategies": {key: value["overall"] for key, value in info["strategies"].items()}}), flush=True)
        db.close()


if __name__ == "__main__":
    main()
