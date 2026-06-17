// GATE 0 part 2 — clang.wasm が使う実 I/O 経路の検証。
// (1) stdin から ustar tar を fd_read で全部読む
// (2) 各メンバを in-wasm WasmFS に展開（fopen/fwrite）
// (3) 焼き込んだ C 配列（sysroot stub 相当）も WasmFS に置く（#220 GATE B' の carray パターン）
// (4) 指定メンバを WasmFS から読み戻して stdout(fd_write) へ出す
// 期待: import は wasi_snapshot_preview1 のみ (env=0)、bare wasmtime（--dir 無し）で成立。
// これが通れば「preopen 不可を stdin-tar / stdout で回避」できることが多時間ビルド前に確定する。
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

// 焼き込みデータ（実運用では wasi-sysroot ヘッダを xxd -i したもの）。ここでは stub。
static const char SYSROOT_STUB[] = "SYSROOT-STUB-DATA-via-Carray";

static unsigned long oct(const char* p, int n) {
    unsigned long v = 0;
    for (int i = 0; i < n && p[i] >= '0' && p[i] <= '7'; i++) v = (v << 3) + (p[i] - '0');
    return v;
}

static void mkparents(const char* path) {
    char tmp[1024];
    strncpy(tmp, path, sizeof(tmp) - 1);
    tmp[sizeof(tmp) - 1] = 0;
    for (char* s = tmp + 1; *s; s++) {
        if (*s == '/') { *s = 0; mkdir(tmp, 0777); *s = '/'; }
    }
}

// stdin を全部メモリへ
static char* slurp_stdin(size_t* out_len) {
    size_t cap = 1 << 20, len = 0;
    char* buf = (char*)malloc(cap);
    size_t n;
    while ((n = fread(buf + len, 1, cap - len, stdin)) > 0) {
        len += n;
        if (len == cap) { cap <<= 1; buf = (char*)realloc(buf, cap); }
    }
    *out_len = len;
    return buf;
}

int main(int argc, char** argv) {
    const char* want = argc > 1 ? argv[1] : "main.cpp";

    // (3) carray を WasmFS に焼き込み配置
    mkparents("/sysroot/stub.txt");
    FILE* sf = fopen("/sysroot/stub.txt", "w");
    if (sf) { fwrite(SYSROOT_STUB, 1, sizeof(SYSROOT_STUB) - 1, sf); fclose(sf); }

    // (1)(2) stdin tar を展開
    size_t len = 0;
    char* tar = slurp_stdin(&len);
    int extracted = 0;
    for (size_t off = 0; off + 512 <= len; ) {
        const char* h = tar + off;
        if (h[0] == 0) break;                       // 終端ブロック
        char name[101] = {0};
        memcpy(name, h, 100);
        unsigned long sz = oct(h + 124, 11);
        char type = h[156];
        off += 512;
        if (type == '0' || type == 0) {             // 通常ファイル
            char path[1100];
            snprintf(path, sizeof(path), "/work/%s", name);
            mkparents(path);
            FILE* w = fopen(path, "w");
            if (w) { fwrite(tar + off, 1, sz, w); fclose(w); extracted++; }
        }
        off += (sz + 511) & ~((size_t)511);          // 512 境界へ
    }
    fprintf(stderr, "EXTRACTED=%d files, tarlen=%zu\n", extracted, len);

    // (4) 指定メンバを WasmFS から読み戻して stdout へ
    char wantpath[1100];
    snprintf(wantpath, sizeof(wantpath), "/work/%s", want);
    FILE* r = fopen(wantpath, "r");
    if (!r) { fprintf(stderr, "member not found: %s\n", wantpath); return 2; }
    char buf[8192];
    size_t n;
    while ((n = fread(buf, 1, sizeof(buf), r)) > 0) fwrite(buf, 1, n, stdout);
    fclose(r);

    // carray が読めることも stderr で確認
    FILE* cr = fopen("/sysroot/stub.txt", "r");
    char cb[64] = {0};
    if (cr) { size_t cn = fread(cb, 1, sizeof(cb) - 1, cr); cb[cn] = 0; fclose(cr); }
    fprintf(stderr, "VIA_CARRAY=[%s]\n", cb);
    return 0;
}
