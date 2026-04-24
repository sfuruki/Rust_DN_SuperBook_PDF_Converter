#!/bin/bash
# Test upscale endpoint

INPUT="/data/work/job_08474aa21746459b94a54df1e1645291/0004/gaozou.webp"
OUTPUT="/tmp/test_0004_result.webp"

echo "Testing upscale on: $INPUT"
echo "Output: $OUTPUT"
echo ""

curl -X POST http://localhost:8000/upscale \
  -H "Content-Type: application/json" \
  -d "{\"input_path\":\"$INPUT\",\"output_path\":\"$OUTPUT\",\"scale\":2}" \
  2>&1

echo ""
echo "Test complete."
