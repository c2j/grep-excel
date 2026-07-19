#!/bin/sh
cd /tmp/bench && ./csvq "SELECT Dept, COUNT(*), AVG(Amount) FROM \`bench.csv\` GROUP BY Dept ORDER BY 2 DESC"
