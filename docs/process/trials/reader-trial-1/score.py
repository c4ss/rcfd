"""Score reader trial 1 against the key: python docs/process/trials/reader-trial-1/score.py"""
import json, os
S = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results")
key = json.load(open(f"{S}/key.json", encoding="utf-8"))
plants = {}
for k in key:
    if k.get("kind") in (None, "none"):
        continue
    plants.setdefault(k["item"], []).append(k)
LISTS = ["results_that_do_not_follow", "checks_that_cannot_fail", "limits_that_test_nothing"]
items = ["F-638", "F-661", "F-595", "F-571", "F-534", "F-639", "F-627", "F-615"]
tot = {}
for it in items:
    for rd in ("same", "other"):
        d = json.load(open(f"{S}/{it}_{rd}.json", encoding="utf-8"))
        fs = [(l, f) for l in LISTS for f in d.get(l, [])]
        hit = []
        for l, f in fs:
            for p in plants.get(it, []):
                if os.path.basename(str(f.get("file", ""))) == p["file"] and abs(int(f.get("line") or 0) - p["line"]) <= 3:
                    hit.append((l, f))
                    break
        n = len(fs)
        t = tot.setdefault(rd, {"found": 0, "plants": 0, "findings": 0, "other": 0})
        t["findings"] += n
        t["other"] += n - len(hit)
        if it in plants:
            t["plants"] += 1
            t["found"] += 1 if hit else 0
        print(f"{it} {rd}: {n} findings, planted={plants[it][0]['kind'] if it in plants else 'none'}, line hits={len(hit)}")
        for l, f in hit:
            print(f"    HIT [{l}] {f['file']}:{f['line']} {f['reason'][:230]}")
        if it in plants and not hit:
            for l, f in fs:
                print(f"    miss? [{l[:7]}] {os.path.basename(str(f.get('file')))[:30]}:{f.get('line')} {f['reason'][:150]}")
print(json.dumps(tot))
