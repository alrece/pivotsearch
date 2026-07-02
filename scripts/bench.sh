#!/usr/bin/env bash
# pivotsearch 性能压测脚本
# 生成 N 个 txt 文件，索引并查询，测延迟基线。
#
# 用法: ./scripts/bench.sh [文件数] [查询次数]
# 默认: 1000 个文件，5 次查询

set -e

COUNT=${1:-1000}
QUERIES=${2:-5}
TMPDIR=$(mktemp -d)
BIN="${BIN:-cargo run -q --release --bin pivotsearch --}"

echo "═══ pivotsearch 性能压测 ═══"
echo "文件数: $COUNT"
echo "测试目录: $TMPDIR"
echo ""

# ── 生成测试文件 ──
echo "【1/4】生成 $COUNT 个 txt 文件..."
START=$(python3 -c "import time;print(time.time())")
for i in $(seq 1 "$COUNT"); do
  # 随机中文内容，含可搜索关键词
  cat > "$TMPDIR/doc_$i.txt" <<EOF
文档编号 $i 的内容。这是一段用于性能测试的中文文本，包含营收、增长、报告、分析等关键词。
随机词：$(python3 -c "import random; print(random.choice(['技术','产品','市场','研发','运营','财务','销售']))") 部门的第 $i 号文档。
EOF
done
GEN_TIME=$(python3 -c "print(f'{$(python3 -c "import time;print(time.time())") - $START:.2f}')")
echo "  生成耗时: ${GEN_TIME}s"

# ── 索引 ──
echo ""
echo "【2/4】索引 $COUNT 个文件..."
START=$(python3 -c "import time;print(time.time())")
cargo run -q --release --bin pivotsearch -- index "$TMPDIR" "$TMPDIR/idx" 2>/dev/null >/dev/null
INDEX_TIME=$(python3 -c "print(f'{$(python3 -c "import time;print(time.time())") - $START:.2f}')")
echo "  索引耗时: ${INDEX_TIME}s"
echo "  平均: $(python3 -c "print(f'{$INDEX_TIME/$COUNT*1000:.1f}')") ms/文件"
echo "  吞吐: $(python3 -c "print(f'{$COUNT/$INDEX_TIME:.0f}')") 文件/秒"

# ── 查询 ──
echo ""
echo "【3/4】查询性能（$QUERIES 次）..."
QUERIES_LIST=("营收" "增长" "技术" "文档" "财务部门")
for q in "${QUERIES_LIST[@]:0:$QUERIES}"; do
  START=$(python3 -c "import time;print(time.time())")
  RESULT=$(cargo run -q --release --bin pivotsearch -- search "$q" "$TMPDIR/idx" 2>/dev/null | grep "命中" | grep -oE "[0-9]+ 条")
  QTIME=$(python3 -c "print(f'{$(python3 -c "import time;print(time.time())") - $START:.3f}')")
  echo "  查询「$q」→ $RESULT, 耗时 ${QTIME}s"
done

# ── 索引体积 ──
echo ""
echo "【4/4】索引体积..."
du -sh "$TMPDIR/idx" 2>/dev/null | awk '{print "  索引体积: " $1}'

echo ""
echo "═══ 压测完成 ═══"
echo "清理测试目录..."
rm -rf "$TMPDIR"
