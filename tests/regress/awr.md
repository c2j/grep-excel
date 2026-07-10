# WORKLOAD REPOSITORY REPORT for

| DB Name   | DB Id      | Instance  | Inst Num | Startup Time      | Release      | RAC |
|-----------|------------|-----------|----------|-------------------|--------------|-----|
| ORCL11G   | 1234567890 | ORCL11G   | 1        | 10-Jul-26 08:00   | 11.2.0.4.0   | NO  |

| Host Name  | Platform           | CPUs | Cores | Sockets | Memory (GB) |
|------------|--------------------|------|-------|---------|-------------|
| dbserver01 | Linux x86 64-bit   | 8    | 4     | 2       | 32          |

|              | Snap Id | Snap Time           | Sessions | Curs/Sess |
|--------------|---------|---------------------|----------|-----------|
| Begin Snap:  | 12345   | 10-Jul-26 14:00:00  | 152      | 2.8       |
| End Snap:    | 12346   | 10-Jul-26 15:00:00  | 148      | 2.9       |
| Elapsed:     |         | 60.12 (mins)        |          |           |
| DB Time:     |         | 245.35 (mins)       |          |           |

---

## Load Profile

|                          | Per Second  | Per Transaction | Per Exec | Per Call |
|--------------------------|-------------|-----------------|----------|----------|
| DB Time(s):              | 4.1         | 0.0             | 0.01     | 0.00     |
| DB CPU(s):               | 2.8         | 0.0             | 0.01     | 0.00     |
| Redo size (bytes):       | 152,384.2   | 1,245.6         |          |          |
| Logical read (blocks):   | 45,672.1    | 373.8           |          |          |
| Block changes:           | 1,234.5     | 10.1            |          |          |
| Physical read (blocks):  | 2,156.3     | 17.6            |          |          |
| Physical write (blocks): | 456.7       | 3.7             |          |          |
| User calls:              | 234.5       | 1.9             |          |          |
| Parses:                  | 156.3       | 1.3             |          |          |
| Hard parses:             | 12.4        | 0.1             |          |          |
| SQL Work Area (MB):      | 2.1         | 0.0             |          |          |
| Logons:                  | 0.8         | 0.0             |          |          |
| Executes:                | 1,245.6     | 10.2            |          |          |
| Rollbacks:               | 2.1         | 0.0             |          |          |
| Transactions:            | 122.3       | 1.0             |          |          |

---

## Instance Efficiency Percentages (Target 100%)

| Statistic                         | Value  | Statistic           | Value   |
|-----------------------------------|--------|---------------------|---------|
| Buffer Nowait %                   | 99.98  | Redo NoWait %         | 100.00  |
| Buffer Hit %                      | 95.28  | In-memory Sort %      | 100.00  |
| Library Hit %                     | 98.45  | Soft Parse %          | 92.07   |
| Execute to Parse %                | 87.45  | Latch Hit %           | 99.85   |
| Parse CPU to Parse Elapsd %       | 72.31  | % Non-Parse CPU       | 95.67   |

---

## Shared Pool Statistics

|                           | Begin Snap | End Snap |
|---------------------------|------------|----------|
| Memory Usage %            | 82.34      | 85.12    |
| % SQL with executions>1   | 78.56      | 79.23    |
| % Memory for SQL w/exec>1 | 65.78      | 66.45    |

---

## Top 5 Timed Foreground Events

| Event                         | Waits     | Time(s) | Avg wait (ms) | % DB time | Wait Class   |
|-------------------------------|-----------|---------|---------------|-----------|--------------|
| db file sequential read       | 456,231   | 4,256   | 9             | **28.9**  | User I/O     |
| DB CPU                        |           | 3,245   |               | 22.0      |              |
| log file sync                 | 89,234    | 2,156   | 24            | 14.6      | Commit       |
| buffer busy waits             | 12,456    | 987     | 79            | 6.7       | Concurrency  |
| latch: cache buffers chains   | 8,923     | 756     | 85            | 5.1       | Concurrency  |

---

## Host CPU

|          | Load Average |        |        |        |        |        |
|----------|--------------|--------|--------|--------|--------|--------|
|          | Begin        | End    | %User  | %System| %WIO   | %Idle  |
|          | 2.45         | 3.12   | 45.2   | 12.3   | 8.7    | 32.1   |

## Instance CPU

| %Total CPU | %Busy CPU | %DB time waiting for CPU (Resource Manager) |
|------------|-----------|-----------------------------------------------|
| 67.8       | 98.2      | 0.0                                           |

---

## Memory Statistics

|                              | Begin      | End        |
|------------------------------|------------|------------|
| Host Mem (MB)                | 32,768.0   | 32,768.0   |
| SGA use (MB)                 | 8,192.0    | 8,192.0    |
| PGA use (MB)                 | 1,536.2    | 1,624.5    |
| % Host Mem used for SGA      | 25.0       | 25.0       |
| % Host Mem used for PGA      | 4.7        | 5.0        |
| % Host Mem used (SGA+PGA)    | 29.7       | 30.0       |

---

## Cache Sizes

|                      | Begin      | End        |                  |         |
|----------------------|------------|------------|------------------|---------|
| Buffer Cache         | 5,504 Mb   | 5,504 Mb   | Std Block Size   | 8 Kb    |
| Shared Pool Size     | 1,792 Mb   | 1,792 Mb   | Log Buffer       | 16,384 Kb|

---

## SQL ordered by Elapsed Time

| Elapsed Time (s) | CPU Time (s) | Executions | Elap per Exec (s) | %Total | SQL Id       | SQL Module       |
|------------------|--------------|------------|-------------------|--------|--------------|------------------|
| 1,245.67         | 987.23       | 12,456     | 0.1000            | 8.45%  | 1a2b3c4d5e6f | JDBC Thin Client |
| 876.54           | 654.32       | 89,234     | 0.0098            | 5.95%  | 7g8h9i0j1k2l | JDBC Thin Client |
| 654.32           | 543.21       | 2,345      | 0.2789            | 4.44%  | 3m4n5o6p7q8r | SQL*Plus         |
| 543.21           | 432.10       | 45,678     | 0.0119            | 3.69%  | 9s0t1u2v3w4x | JDBC Thin Client |
| 432.10           | 321.09       | 1,234      | 0.3502            | 2.93%  | 5y6z7a8b9c0d | SQL*Plus         |

```sql
SELECT /*+ INDEX(t idx_emp_dept) */ emp_id, emp_name, salary
FROM employees t
WHERE dept_id = :1 AND hire_date > :2
ORDER BY salary DESC
```

```sql
UPDATE accounts
SET balance = balance + :1, last_modified = SYSDATE
WHERE account_id = :2
```

```sql
BEGIN process_monthly_billing(:1, :2, :3); END;
```

```sql
INSERT INTO audit_log (log_id, action, user_id, created_at)
VALUES (audit_seq.NEXTVAL, :1, :2, SYSDATE)
```

```sql
SELECT * FROM (
  SELECT t.*, ROW_NUMBER() OVER (ORDER BY created_at DESC) rn
  FROM large_table t
) WHERE rn BETWEEN :1 AND :2
```

---

## SQL ordered by CPU Time

| CPU Time (s) | Elapsed Time (s) | Executions | CPU per Exec (s) | %Total | SQL Id       |
|--------------|------------------|------------|------------------|--------|--------------|
| 987.23       | 1,245.67         | 12,456     | 0.0792           | 12.3%  | 1a2b3c4d5e6f |
| 654.32       | 876.54           | 89,234     | 0.0073           | 8.2%   | 7g8h9i0j1k2l |
| 543.21       | 654.32           | 2,345      | 0.2316           | 6.8%   | 3m4n5o6p7q8r |
| 432.10       | 543.21           | 45,678     | 0.0095           | 5.4%   | 9s0t1u2v3w4x |
| 321.09       | 432.10           | 1,234      | 0.2602           | 4.0%   | 5y6z7a8b9c0d |

---

## SQL ordered by Gets

| Gets       | CPU     | Elapsed  | Per Exec   | %Total | SQL Id       |
|------------|---------|----------|------------|--------|--------------|
| 45,678,901 | 543.21  | 654.32   | 19,479.28  | 15.2%  | 3m4n5o6p7q8r |
| 32,109,876 | 987.23  | 1,245.67 | 2,579.79   | 10.7%  | 1a2b3c4d5e6f |
| 21,098,765 | 432.10  | 543.21   | 462.17     | 7.0%   | 9s0t1u2v3w4x |
| 12,345,678 | 654.32  | 876.54   | 138.37     | 4.1%   | 7g8h9i0j1k2l |
| 8,765,432  | 210.98  | 321.09   | 8,765.43   | 2.9%   | 2e3f4g5h6i7j |

---

## SQL ordered by Reads

| Physical Reads | Executions | Reads per Exec | %Total | CPU Time (s) | Elapsed Time (s) | SQL Id       |
|----------------|------------|----------------|--------|--------------|------------------|--------------|
| 2,345,678      | 1,234      | 1,900.87       | 45.2%  | 210.98       | 321.09           | 2e3f4g5h6i7j |
| 1,234,567      | 45,678     | 27.03          | 23.8%  | 432.10       | 543.21           | 9s0t1u2v3w4x |
| 987,654        | 2,345      | 421.18         | 19.0%  | 543.21       | 654.32           | 3m4n5o6p7q8r |
| 456,789        | 12,456     | 36.67          | 8.8%   | 987.23       | 1,245.67         | 1a2b3c4d5e6f |
| 123,456        | 89,234     | 1.38           | 2.4%   | 654.32       | 876.54           | 7g8h9i0j1k2l |

---

## SQL ordered by Executions

| Executions | Rows Processed | Rows per Exec | CPU per Exec (s) | Elap per Exec (s) | SQL Id       |
|------------|----------------|---------------|------------------|-------------------|--------------|
| 89,234     | 89,234         | 1.0000        | 0.0073           | 0.0098            | 7g8h9i0j1k2l |
| 45,678     | 45,678         | 1.0000        | 0.0095           | 0.0119            | 9s0t1u2v3w4x |
| 12,456     | 12,456         | 1.0000        | 0.0792           | 0.1000            | 1a2b3c4d5e6f |
| 2,345      | 2,345          | 1.0000        | 0.2316           | 0.2789            | 3m4n5o6p7q8r |
| 1,234      | 1,234          | 1.0000        | 0.2602           | 0.3502            | 5y6z7a8b9c0d |

---

## Instance Activity Stats

| Statistic                                      | Total           | per Second | per Trans |
|------------------------------------------------|-----------------|------------|-----------|
| CPU used by this session                       | 168,432.0       | 46.7       | 0.4       |
| CPU used when call started                     | 165,789.0       | 46.0       | 0.4       |
| CR blocks created                              | 12,345.0        | 3.4        | 0.0       |
| Cached Commit SCN referenced                   | 456,789.0       | 127.0      | 1.0       |
| Commit SCN cached                              | 123,456.0       | 34.3       | 0.3       |
| DB time                                        | 245,350.0       | 68.1       | 0.6       |
| DBWR checkpoint buffers written                | 89,234.0        | 24.8       | 0.2       |
| DBWR transaction table writes                  | 12,456.0        | 3.5        | 0.0       |
| DBWR undo block writes                         | 34,567.0        | 9.6        | 0.1       |
| SQL*Net roundtrips to/from client              | 1,234,567.0     | 342.9      | 2.8       |
| buffer is not pinned count                     | 12,345,678.0    | 3,429.6    | 28.1      |
| buffer is pinned count                         | 34,567,890.0    | 9,602.2    | 78.6      |
| bytes received via SQL*Net from client         | 45,678,901.0    | 12,688.6   | 103.9     |
| bytes sent via SQL*Net to client               | 123,456,789.0   | 34,293.6   | 280.9     |
| calls to get snapshot scn: kcmgss              | 456,789.0       | 127.0      | 1.0       |
| change write time                              | 1,234.0         | 0.3        | 0.0       |
| cleanouts and rollbacks - consistent           | 12,345.0        | 3.4        | 0.0       |
| cluster wait time                              | 1,234.0         | 0.3        | 0.0       |
| commit batch performed                         | 89,234.0        | 24.8       | 0.2       |
| commit cleanouts                               | 123,456.0       | 34.3       | 0.3       |
| commit cleanouts successfully completed        | 89,234.0        | 24.8       | 0.2       |
| concurrency wait time                          | 987.0           | 0.3        | 0.0       |
| consistent changes                             | 12,345.0        | 3.4        | 0.0       |
| consistent gets                                | 4,567,890.0     | 1,268.9    | 10.4      |
| consistent gets - examination                  | 1,234,567.0     | 343.2      | 2.8       |
| consistent gets from cache                     | 4,565,545.0     | 1,268.2    | 10.4      |
| db block changes                               | 123,456.0       | 34.3       | 0.3       |
| db block gets                                  | 456,789.0       | 127.0      | 1.0       |
| db block gets from cache                       | 456,789.0       | 127.0      | 1.0       |
| deferred (CURRENT) block cleanout applied      | 12,345.0        | 3.4        | 0.0       |
| dirty buffers inspected                        | 1,234.0         | 0.3        | 0.0       |
| enqueue deadlocks                              | 2.0             | 0.0        | 0.0       |
| enqueue releases                               | 89,234.0        | 24.8       | 0.2       |
| enqueue requests                               | 89,234.0        | 24.8       | 0.2       |
| execute count                                  | 1,245,678.0     | 345.8      | 2.8       |
| free buffer inspected                          | 2,345.0         | 0.7        | 0.0       |
| free buffer requested                          | 12,345.0        | 3.4        | 0.0       |
| immediate (CR) block cleanout applied            | 12,345.0        | 3.4        | 0.0       |
| index fast full scans (full)                   | 2,345.0         | 0.7        | 0.0       |
| index fetch by key                             | 45,678.0        | 12.7       | 0.1       |
| index scans kdiixs1                            | 12,345.0        | 3.4        | 0.0       |
| leaf node 90-10 splits                         | 456.0           | 0.1        | 0.0       |
| leaf node splits                               | 1,234.0         | 0.3        | 0.0       |
| lob reads                                      | 2,345.0         | 0.7        | 0.0       |
| lob writes                                     | 1,234.0         | 0.3        | 0.0       |
| logons cumulative                              | 12,456.0        | 3.5        | 0.0       |
| messages received                              | 89,234.0        | 24.8       | 0.2       |
| messages sent                                  | 89,234.0        | 24.8       | 0.2       |
| no work - consistent read gets                 | 4,567,890.0     | 1,268.9    | 10.4      |
| opened cursors cumulative                      | 12,345.0        | 3.4        | 0.0       |
| parse count (hard)                             | 12,456.0        | 3.5        | 0.0       |
| parse count (total)                            | 89,234.0        | 24.8       | 0.2       |
| parse time cpu                                 | 1,234.0         | 0.3        | 0.0       |
| parse time elapsed                             | 1,876.0         | 0.5        | 0.0       |
| physical read IO requests                      | 12,345.0        | 3.4        | 0.0       |
| physical read bytes                            | 1,234,567,890.0 | 343,046.6  | 2,808.6   |
| physical read total IO requests                | 12,456.0        | 3.5        | 0.0       |
| physical read total bytes                      | 1,345,678,901.0 | 373,799.1  | 3,055.4   |
| physical reads                                 | 2,345.0         | 0.7        | 0.0       |
| physical reads cache                           | 1,234.0         | 0.3        | 0.0       |
| physical reads direct                          | 1,111.0         | 0.3        | 0.0       |
| physical write IO requests                     | 456.0           | 0.1        | 0.0       |
| physical write bytes                           | 123,456,789.0   | 34,293.6   | 280.9     |
| physical write total IO requests               | 789.0           | 0.2        | 0.0       |
| physical write total bytes                     | 234,567,890.0   | 65,157.7   | 533.6     |
| physical writes                                | 2,345.0         | 0.7        | 0.0       |
| physical writes direct                         | 456.0           | 0.1        | 0.0       |
| physical writes from cache                     | 1,889.0         | 0.5        | 0.0       |
| prefetched blocks                              | 2,345.0         | 0.7        | 0.0       |
| recursive calls                                | 123,456.0       | 34.3       | 0.3       |
| recursive cpu usage                            | 2,345.0         | 0.7        | 0.0       |
| redo blocks written                            | 89,234.0        | 24.8       | 0.2       |
| redo buffer allocation retries                 | 2.0             | 0.0        | 0.0       |
| redo entries                                   | 123,456.0       | 34.3       | 0.3       |
| redo log space requests                        | 456.0           | 0.1        | 0.0       |
| redo log space wait time                       | 123.0           | 0.0        | 0.0       |
| redo ordering marks                            | 2,345.0         | 0.7        | 0.0       |
| redo size                                      | 152,384,200.0   | 42,384.5   | 346.7     |
| redo subscn max counts                         | 1,234.0         | 0.3        | 0.0       |
| redo synch time                                | 2,345.0         | 0.7        | 0.0       |
| redo synch writes                              | 89,234.0        | 24.8       | 0.2       |
| redo wastage                                   | 12,345,678.0    | 3,429.6    | 28.1      |
| redo write time                                | 1,234.0         | 0.3        | 0.0       |
| redo writes                                    | 89,234.0        | 24.8       | 0.2       |
| rollback changes - undo records applied          | 12,345.0        | 3.4        | 0.0       |
| rollbacks only - consistent read gets            | 2,345.0         | 0.7        | 0.0       |
| rows fetched via hash                          | 45,678.0        | 12.7       | 0.1       |
| session connect time                           | 1,234.0         | 0.3        | 0.0       |
| session cursor cache hits                      | 45,678.0        | 12.7       | 0.1       |
| session logical reads                          | 4,567,890.0     | 1,268.9    | 10.4      |
| session pga memory max                         | 123,456,789.0   | 34,293.6   | 280.9     |
| session uga memory                             | 12,345,678.0    | 3,429.6    | 28.1      |
| session uga memory max                         | 23,456,789.0    | 6,515.8    | 53.3      |
| sorts (disk)                                   | 1,234.0         | 0.3        | 0.0       |
| sorts (memory)                                 | 45,678.0        | 12.7       | 0.1       |
| sorts (rows)                                   | 456,789.0       | 127.0      | 1.0       |
| table fetch by rowid                           | 45,678.0        | 12.7       | 0.1       |
| table fetch continued row                      | 2,345.0         | 0.7        | 0.0       |
| table scan blocks gotten                       | 12,345.0        | 3.4        | 0.0       |
| table scan rows gotten                         | 123,456.0       | 34.3       | 0.3       |
| table scans (direct read)                      | 456.0           | 0.1        | 0.0       |
| table scans (long tables)                      | 1,234.0         | 0.3        | 0.0       |
| table scans (short tables)                     | 45,678.0        | 12.7       | 0.1       |
| transaction rollbacks                          | 2,345.0         | 0.7        | 0.0       |
| user calls                                     | 234,567.0       | 65.2       | 0.5       |
| user commits                                   | 89,234.0        | 24.8       | 0.2       |
| user rollbacks                                 | 2,345.0         | 0.7        | 0.0       |
| workarea executions - optimal                  | 45,678.0        | 12.7       | 0.1       |
| workarea executions - onepass                  | 1,234.0         | 0.3        | 0.0       |
| workarea executions - multipass                | 456.0           | 0.1        | 0.0       |
| write clones created in foreground             | 1,234.0         | 0.3        | 0.0       |

---

## IO Stats by tablespace

| TS# | Name      | Reads     | Av Reads/s | Av Rd(ms) | Av Blks/Rd | Writes  | Av Writes/s | Buffer Waits | Av Buf Wt(ms) |
|-----|-----------|-----------|------------|-----------|------------|---------|-------------|--------------|---------------|
| 0   | SYSTEM    | 123,456   | 34.3       | 8.5       | 1.0        | 12,345  | 3.4         | 123          | 2.1           |
| 1   | SYSAUX    | 45,678    | 12.7       | 12.3      | 1.0        | 2,345   | 0.7         | 45           | 3.2           |
| 2   | UNDOTBS1  | 23,456    | 6.5        | 5.2       | 1.0        | 89,234  | 24.8        | 12           | 1.8           |
| 3   | TEMP      | 2,345     | 0.7        | 3.1       | 7.5        | 456     | 0.1         | 0            | 0.0           |
| 4   | USERS     | 456,789   | 127.0      | 9.8       | 1.0        | 34,567  | 9.6         | 456          | 8.7           |
| 5   | EXAMPLE   | 1,234     | 0.3        | 4.2       | 1.0        | 123     | 0.0         | 2            | 1.2           |

---

## IO Stats by file

| File# | Name                                               | Tablespace | Reads     | Av Reads/s | Av Rd(ms) | Av Blks/Rd | Writes  | Av Writes/s | Buffer Waits | Av Buf Wt(ms) |
|-------|----------------------------------------------------|------------|-----------|------------|-----------|------------|---------|-------------|--------------|---------------|
| 1     | /u01/app/oracle/oradata/orcl11g/system01.dbf      | SYSTEM     | 123,456   | 34.3       | 8.5       | 1.0        | 12,345  | 3.4         | 123          | 2.1           |
| 2     | /u01/app/oracle/oradata/orcl11g/sysaux01.dbf      | SYSAUX     | 45,678    | 12.7       | 12.3      | 1.0        | 2,345   | 0.7         | 45           | 3.2           |
| 3     | /u01/app/oracle/oradata/orcl11g/undotbs01.dbf     | UNDOTBS1   | 23,456    | 6.5        | 5.2       | 1.0        | 89,234  | 24.8        | 12           | 1.8           |
| 4     | /u01/app/oracle/oradata/orcl11g/temp01.dbf        | TEMP       | 2,345     | 0.7        | 3.1       | 7.5        | 456     | 0.1         | 0            | 0.0           |
| 5     | /u01/app/oracle/oradata/orcl11g/users01.dbf       | USERS      | 456,789   | 127.0      | 9.8       | 1.0        | 34,567  | 9.6         | 456          | 8.7           |
| 6     | /u01/app/oracle/oradata/orcl11g/example01.dbf     | EXAMPLE    | 1,234     | 0.3        | 4.2       | 1.0        | 123     | 0.0         | 2            | 1.2           |

---

## Buffer Pool Statistics

| Name        | Pinned Buffers | Old Buffers | Total Buffers | Total Requests | % of Waits | % of Gets | % of Reads | % of Time |
|-------------|----------------|-------------|---------------|----------------|------------|-----------|------------|-----------|
| DEFAULT 32K | 0              | 0           | 0             | 0              | 0.0        | 0.0       | 0.0        | 0.0       |
| DEFAULT 16K | 0              | 0           | 0             | 0              | 0.0        | 0.0       | 0.0        | 0.0       |
| DEFAULT 8K  | 12,345         | 45,678      | 123,456       | 4,567,890      | 0.1        | 99.9      | 95.3       | 0.1       |
| DEFAULT 4K  | 0              | 0           | 0             | 0              | 0.0        | 0.0       | 0.0        | 0.0       |
| DEFAULT 2K  | 0              | 0           | 0             | 0              | 0.0        | 0.0       | 0.0        | 0.0       |

---

## Advisory Statistics

### Instance Recovery Stats

| Target MTTR (s) | Estimated MTTR (s) |
|-----------------|--------------------|
| 0               | 12                 |

### Buffer Pool Advisory

| Size for Est (MB) | Size Factor | Buffers   | Physical Reads | Est Phys Read Factor |
|-------------------|-------------|-----------|----------------|----------------------|
| 4,928             | 0.90        | 616,000   | 2,456,789      | 1.12                 |
| 5,120             | 0.93        | 640,000   | 2,234,567      | 1.02                 |
| 5,312             | 0.96        | 664,000   | 2,123,456      | 0.97                 |
| 5,504             | 1.00        | 688,000   | 2,098,765      | 0.96                 |
| 5,696             | 1.03        | 712,000   | 2,087,654      | 0.95                 |
| 5,888             | 1.07        | 736,000   | 2,076,543      | 0.94                 |
| 6,080             | 1.10        | 760,000   | 2,065,432      | 0.94                 |

### PGA Memory Advisory

| PGA Target Est (MB) | Size Factor | W/A MB Processed | Estd Extra W/A MB | Estd PGA Cache Hit % | Estd PGA Overalloc |
|---------------------|-------------|------------------|-------------------|----------------------|--------------------|
| 384                 | 0.13        | 1,245.6          | 456.7             | 73.2                 | 1.0                |
| 768                 | 0.25        | 1,245.6          | 234.5             | 86.1                 | 0.0                |
| 1,536               | 0.50        | 1,245.6          | 89.2              | 94.7                 | 0.0                |
| 2,304               | 0.75        | 1,245.6          | 34.5              | 97.9                 | 0.0                |
| 3,072               | 1.00        | 1,245.6          | 12.3              | 99.2                 | 0.0                |
| 3,686               | 1.20        | 1,245.6          | 4.5               | 99.7                 | 0.0                |
| 4,608               | 1.50        | 1,245.6          | 1.2               | 99.9                 | 0.0                |

---

## Wait Event Histogram

| Event                          | Waits     | <1ms | <2ms | <4ms | <8ms | <16ms | <32ms | <=1s | >1s |
|--------------------------------|-----------|------|------|------|------|-------|-------|------|-----|
| buffer busy waits              | 12,456    | 45.2 | 23.4 | 15.6 | 9.8  | 4.5   | 1.2   | 0.3  | 0.0 |
| db file parallel read          | 2,345     | 12.3 | 15.6 | 23.4 | 28.9 | 14.5  | 4.2   | 1.1  | 0.0 |
| db file parallel write         | 89,234    | 67.8 | 18.9 | 8.7  | 3.2  | 1.1   | 0.3   | 0.0  | 0.0 |
| db file scattered read         | 45,678    | 23.4 | 28.9 | 19.8 | 15.6 | 8.7   | 3.2   | 0.4  | 0.0 |
| db file sequential read        | 456,231   | 34.5 | 28.7 | 19.8 | 10.2 | 5.1   | 1.4   | 0.3  | 0.0 |
| direct path read               | 12,345    | 56.7 | 23.4 | 12.3 | 5.6  | 1.8   | 0.2   | 0.0  | 0.0 |
| direct path read temp          | 2,345     | 67.8 | 18.9 | 8.7  | 3.2  | 1.1   | 0.3   | 0.0  | 0.0 |
| direct path write              | 1,234     | 45.6 | 23.4 | 15.6 | 9.8  | 4.5   | 1.1   | 0.0  | 0.0 |
| enq: HW - contention           | 123       | 23.4 | 15.6 | 12.3 | 18.9 | 19.8  | 8.7   | 1.3  | 0.0 |
| enq: SQ - contention           | 234       | 34.5 | 23.4 | 15.6 | 12.3 | 10.2  | 3.2   | 0.8  | 0.0 |
| enq: TX - index contention     | 456       | 12.3 | 8.7  | 5.6  | 4.5  | 3.2   | 1.8   | 0.9  | 0.0 |
| enq: TX - row lock contention  | 789       | 45.6 | 23.4 | 15.6 | 9.8  | 4.5   | 1.1   | 0.0  | 0.0 |
| latch: cache buffers chains    | 8,923     | 23.4 | 15.6 | 12.3 | 18.9 | 19.8  | 8.7   | 1.3  | 0.0 |
| latch: redo allocation         | 456       | 34.5 | 23.4 | 15.6 | 12.3 | 10.2  | 3.2   | 0.8  | 0.0 |
| latch: shared pool             | 123       | 45.6 | 23.4 | 15.6 | 9.8  | 4.5   | 1.1   | 0.0  | 0.0 |
| latch: library cache           | 234       | 56.7 | 23.4 | 12.3 | 5.6  | 1.8   | 0.2   | 0.0  | 0.0 |
| latch: library cache pin       | 345       | 67.8 | 18.9 | 8.7  | 3.2  | 1.1   | 0.3   | 0.0  | 0.0 |
| latch: library cache lock      | 456       | 78.9 | 12.3 | 5.6  | 2.3  | 0.9   | 0.0   | 0.0  | 0.0 |
| latch: row cache objects       | 567       | 89.0 | 7.8  | 2.3  | 0.9  | 0.0   | 0.0   | 0.0  | 0.0 |
| library cache load lock        | 123       | 45.6 | 23.4 | 15.6 | 9.8  | 4.5   | 1.1   | 0.0  | 0.0 |
| library cache pin              | 234       | 56.7 | 23.4 | 12.3 | 5.6  | 1.8   | 0.2   | 0.0  | 0.0 |
| library cache lock             | 345       | 67.8 | 18.9 | 8.7  | 3.2  | 1.1   | 0.3   | 0.0  | 0.0 |
| log file parallel write        | 89,234    | 78.9 | 12.3 | 5.6  | 2.3  | 0.9   | 0.0   | 0.0  | 0.0 |
| log file sequential read       | 2,345     | 89.0 | 7.8  | 2.3  | 0.9  | 0.0   | 0.0   | 0.0  | 0.0 |
| log file single write          | 456       | 90.1 | 6.5  | 2.3  | 0.9  | 0.2   | 0.0   | 0.0  | 0.0 |
| log file sync                  | 89,234    | 12.3 | 8.7  | 5.6  | 4.5  | 3.2   | 1.8   | 0.9  | 0.0 |
| os thread startup              | 123       | 23.4 | 15.6 | 12.3 | 18.9 | 19.8  | 8.7   | 1.3  | 0.0 |
| rdbms ipc message              | 456,789   | 34.5 | 23.4 | 15.6 | 12.3 | 10.2  | 3.2   | 0.8  | 0.0 |
| read by other session          | 12,345    | 45.6 | 23.4 | 15.6 | 9.8  | 4.5   | 1.1   | 0.0  | 0.0 |
| reliable message               | 2,345     | 56.7 | 23.4 | 12.3 | 5.6  | 1.8   | 0.2   | 0.0  | 0.0 |
| SQL*Net break/reset to client  | 456       | 67.8 | 18.9 | 8.7  | 3.2  | 1.1   | 0.3   | 0.0  | 0.0 |
| SQL*Net message from client    | 1,234,567 | 78.9 | 12.3 | 5.6  | 2.3  | 0.9   | 0.0   | 0.0  | 0.0 |
| SQL*Net message to client      | 1,234,567 | 89.0 | 7.8  | 2.3  | 0.9  | 0.0   | 0.0   | 0.0  | 0.0 |
| SQL*Net more data from client  | 12,345    | 90.1 | 6.5  | 2.3  | 0.9  | 0.2   | 0.0   | 0.0  | 0.0 |
| SQL*Net more data to client    | 23,456    | 91.2 | 5.6  | 2.3  | 0.9  | 0.0   | 0.0   | 0.0  | 0.0 |

---

## Operating System Statistics

| Statistic                  | Total           |
|----------------------------|-----------------|
| AVG_BUSY_TIME              | 456,789         |
| AVG_IDLE_TIME              | 123,456         |
| AVG_IOWAIT_TIME            | 89,234          |
| AVG_SYS_TIME               | 234,567         |
| AVG_USER_TIME              | 345,678         |
| BUSY_TIME                  | 3,654,321       |
| IDLE_TIME                  | 1,234,567       |
| IOWAIT_TIME                | 987,654         |
| SYS_TIME                   | 1,876,543       |
| USER_TIME                  | 2,456,789       |
| OS_CPU_WAIT_TIME           | 123,456         |
| RSRC_MGR_CPU_WAIT_TIME     | 0               |
| PHYSICAL_MEMORY_BYTES      | 32,768,000,000  |
| NUM_CPUS                   | 8               |
| NUM_CPU_CORES              | 4               |
| NUM_CPU_SOCKETS            | 2               |
| TCP_RECEIVE_SIZE_DEFAULT   | 8,192           |
| TCP_SEND_SIZE_DEFAULT      | 8,192           |

---

**End of Report**
