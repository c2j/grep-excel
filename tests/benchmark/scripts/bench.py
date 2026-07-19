#!/usr/bin/env python3
"""基准测试：对指定命令列表计时，stdout 重定向到 /dev/null，取 min/median。
用法: python3 bench.py '<json array of {name, cmd, runs?, timeout?}>' [默认超时秒]"""
import subprocess, time, sys, json, statistics, os

DEVNULL = open(os.devnull, "w")

def run(cmd, runs=3, warmup=1, shell=True, timeout=300):
    times = []
    for i in range(warmup + runs):
        t0 = time.perf_counter()
        try:
            r = subprocess.run(cmd, shell=shell, stdout=DEVNULL, stderr=DEVNULL, timeout=timeout)
            dt = time.perf_counter() - t0
            if r.returncode != 0:
                return {"cmd": cmd, "error": f"exit={r.returncode}"}
        except subprocess.TimeoutExpired:
            return {"cmd": cmd, "error": f"timeout>{timeout}s"}
        if i >= warmup:
            times.append(dt)
    return {"cmd": cmd, "min": round(min(times), 3), "median": round(statistics.median(times), 3)}

if __name__ == "__main__":
    jobs = json.loads(sys.argv[1])
    timeout = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    for j in jobs:
        res = run(j["cmd"], runs=j.get("runs", 3), warmup=j.get("warmup", 1), timeout=j.get("timeout", timeout))
        res["name"] = j["name"]
        if "error" in res:
            print(f'{j["name"]:24s} ERROR {res["error"]}', flush=True)
        else:
            print(f'{j["name"]:24s} min={res["min"]:8.3f}s  median={res["median"]:8.3f}s', flush=True)
