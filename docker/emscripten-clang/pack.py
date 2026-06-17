#!/usr/bin/env python3
# stdin-tar の代わりの単純・堅牢なアーカイブ形式を作る。
# 各ファイルを  "<abs-path>\t<size>\n" + <size bytes>  で連結して stdout へ。
# tar の 512 境界 / 8進サイズ / 100文字名上限などのエッジケースを排除する。
# 使い方: pack.py <root-dir> > stream.bin   （root 配下を絶対パス /xxx で格納）
import os, sys
root = os.path.abspath(sys.argv[1])
out = sys.stdout.buffer
for dirpath, _dirs, files in os.walk(root):
    for f in sorted(files):
        full = os.path.join(dirpath, f)
        rel = "/" + os.path.relpath(full, root).replace(os.sep, "/")
        with open(full, "rb") as fh:
            data = fh.read()
        out.write(("%s\t%d\n" % (rel, len(data))).encode("utf-8"))
        out.write(data)
out.flush()
