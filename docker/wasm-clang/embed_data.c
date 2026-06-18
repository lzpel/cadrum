// clang.wasm に焼き込む constructor。WASMFS は WASI preopen(--dir)を見ず、standalone の
// stdin は大きい入力で破綻する（emscripten #23724 / #21335）。そこで入力一式（ヘッダ＋
// ソース）を `.incbin` で wasm のデータセクションに焼き込み（embed_data.S）、起動時に
// その in-memory blob を WASMFS へ展開する。clang は WASMFS 上のファイルを読み、出力 .o は
// `-o -`(stdout) で受ける。preopen 不可・stdin 不可の standalone WASMFS で入力受け渡しを
// 成立させる #220「次段 A」の実装（sysroot/プロジェクトの焼き込み）。
//
// blob 形式（pack.py が生成）: 各エントリ  "<abs-path>\t<size>\n" + <size bytes> の連結。
//
// __attribute__((constructor)) は __wasm_call_ctors（_start 内・main 前）で走る。
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

// embed_data.c が #embed で定義する焼き込み blob。
extern const unsigned char cadrum_blob_start[];
extern const unsigned long cadrum_blob_len;

static void mkparents_(const char* path) {
    char tmp[4096];
    strncpy(tmp, path, sizeof(tmp) - 1);
    tmp[sizeof(tmp) - 1] = 0;
    for (char* s = tmp + 1; *s; s++)
        if (*s == '/') { *s = 0; mkdir(tmp, 0777); *s = '/'; }
}

__attribute__((constructor))
static void extract_embedded_blob(void) {
    const char* buf = (const char*)cadrum_blob_start;
    size_t len = (size_t)cadrum_blob_len;
    if (len == 0) return;

    int extracted = 0;
    size_t pos = 0;
    while (pos < len) {
        const char* nl = (const char*)memchr(buf + pos, '\n', len - pos);
        if (!nl) break;
        const char* tab = (const char*)memchr(buf + pos, '\t', nl - (buf + pos));
        if (!tab) break;
        char path[4096];
        size_t plen = tab - (buf + pos);
        if (plen >= sizeof(path)) break;
        memcpy(path, buf + pos, plen);
        path[plen] = 0;
        size_t sz = (size_t)strtoull(tab + 1, NULL, 10);
        size_t data = (nl - buf) + 1;
        if (data + sz > len) break;
        mkparents_(path);
        FILE* w = fopen(path, "wb");
        if (w) { if (sz) fwrite(buf + data, 1, sz, w); fclose(w); extracted++; }
        pos = data + sz;
    }
    fprintf(stderr, "[embed] extracted %d files (%zu bytes) from baked blob into WASMFS\n", extracted, len);
}

// 出力 .o の取り出し。WASMFS standalone は clang の raw バイナリ stdout 書き込みで 0x00 が
// 化ける（putchar 経由は無事なので raw fd 書き込み経路の問題）。そこで clang には
// `-o /tmp/cadrum_out.o` でファイル出力させ、終了時にそれを hex で stdout に吐く（hex は
// 0x00 を含まず印字可能 ASCII のみで安全）。ホスト側で `xxd -r -p` で復元する。
__attribute__((destructor))
static void emit_output_as_hex(void) {
    FILE* f = fopen("/tmp/cadrum_out.o", "rb");
    if (!f) return;
    static const char hx[] = "0123456789abcdef";
    int c;
    while ((c = fgetc(f)) != EOF) { putchar(hx[(c >> 4) & 0xf]); putchar(hx[c & 0xf]); }
    fclose(f);
    fflush(stdout);
}
