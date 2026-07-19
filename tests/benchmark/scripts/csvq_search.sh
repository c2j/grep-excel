#!/bin/sh
cd /tmp/bench && ./csvq "SELECT * FROM \`bench.csv\` WHERE Note LIKE '%ZXQ-7734%'"
