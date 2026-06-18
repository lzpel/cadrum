// clang.wasm 内蔵 bridge。emscripten WASMFS は wasmtime の `--dir` preopen を見ない
// （"/" が memory backend 固定）が、WASI import を直接叩けば preopen fd を触れる（GATE 1a で実証）。
//   - constructor: 全 preopen を再帰列挙し、その内容を MEMFS(WASMFS) へコピー。
//     以後 clang は通常の fopen で host のソース/ヘッダを読める（焼き込み不要）。
//   - destructor:  出力 preopen（guest 名 "/out"）配下に MEMFS のファイルを書き戻す
//     （raw file write は 0x00 を化けさせない）。
//
// `<wasi/api.h>` の __wasi_* は「libc 実装」を期待し emscripten が path_open/fd_readdir を
// 提供しないためリンク不可。よって **import_module="wasi_snapshot_preview1" を明示した自前宣言**
// を使う（全て純 WASI import＝env=0 維持）。clang リンクの jsifier が undefined 扱いするのは
// ビルド側の `-sERROR_ON_UNDEFINED_SYMBOLS=0` で抑止する。
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <dirent.h>

typedef unsigned char  u8;
typedef unsigned short u16;
typedef unsigned int   u32;
typedef unsigned long long u64;
#define WI(n) __attribute__((import_module("wasi_snapshot_preview1"), import_name(n)))

WI("fd_prestat_get")      int w_prestat_get(int fd, void* prestat);
WI("fd_prestat_dir_name") int w_prestat_name(int fd, char* path, u32 len);
WI("path_open")           int w_path_open(int dirfd, u32 dirflags, const char* path, u32 path_len,
                                          u16 oflags, u64 rb, u64 ri, u16 fdflags, int* opened);
WI("fd_readdir")          int w_readdir(int fd, void* buf, u32 buf_len, u64 cookie, u32* used);
WI("fd_read")             int w_read(int fd, const void* iovs, u32 n, u32* nread);
WI("fd_write")            int w_write(int fd, const void* iovs, u32 n, u32* nwrote);
WI("fd_close")            int w_close(int fd);

typedef struct { void* buf; u32 len; } iov;
typedef struct { u8 tag; u8 _p[3]; u32 name_len; } prestat;          // __wasi_prestat_t
typedef struct { u64 d_next; u64 d_ino; u32 d_namlen; u8 d_type; u8 _p[3]; } wdirent; // 24B

#define LOOKUP_FOLLOW 1
#define OF_CREAT 1
#define OF_DIRECTORY 2
#define OF_TRUNC 8
#define FT_DIR 3
// rights (__WASI_RIGHTS_*)
#define R_READ     (1ull<<1)
#define R_SEEK     (1ull<<2)
#define R_TELL     (1ull<<5)
#define R_WRITE    (1ull<<6)
#define R_CREATE   (1ull<<9)
#define R_PATHOPEN (1ull<<13)
#define R_READDIR  (1ull<<14)
#define R_PFSTAT   (1ull<<18)
#define R_FSTAT    (1ull<<21)
#define RD (R_READ|R_SEEK|R_TELL|R_FSTAT|R_READDIR|R_PATHOPEN|R_PFSTAT)
#define WR (R_WRITE|R_SEEK|R_TELL|R_FSTAT|R_PATHOPEN|R_CREATE|R_PFSTAT)

static void mkparents(const char* path) {
    char t[4096]; strncpy(t, path, sizeof t - 1); t[sizeof t - 1] = 0;
    for (char* s = t + 1; *s; s++) if (*s == '/') { *s = 0; mkdir(t, 0777); *s = '/'; }
}

static void copy_in_file(int dirfd, const char* rel, const char* memfs_path) {
    int f = -1;
    if (w_path_open(dirfd, LOOKUP_FOLLOW, rel, strlen(rel), 0, RD, RD, 0, &f) != 0) return;
    mkparents(memfs_path);
    FILE* w = fopen(memfs_path, "wb");
    if (!w) { w_close(f); return; }
    static char buf[65536];
    for (;;) {
        iov v = { buf, sizeof buf }; u32 n = 0;
        if (w_read(f, &v, 1, &n) != 0 || n == 0) break;
        fwrite(buf, 1, n, w);
    }
    fclose(w); w_close(f);
}

static void walk_dir(int dirfd, const char* prefix) {
    mkparents(prefix); mkdir(prefix, 0777);
    char dbuf[65536];   // 非 static（再帰で共有すると外側ループが壊れる）
    u64 cookie = 0;
    for (;;) {
        u32 used = 0;
        if (w_readdir(dirfd, dbuf, sizeof dbuf, cookie, &used) != 0 || used == 0) break;
        u32 off = 0; int progressed = 0;
        while (off + sizeof(wdirent) <= used) {
            wdirent* e = (wdirent*)(dbuf + off);
            u32 nl = e->d_namlen;
            if (off + sizeof(wdirent) + nl > used) break; // 名前切れ→次バッチ
            u64 next = e->d_next; u8 type = e->d_type;
            char name[1024];
            if (nl < sizeof name) {
                memcpy(name, dbuf + off + sizeof(wdirent), nl); name[nl] = 0;
                if (strcmp(name, ".") && strcmp(name, "..")) {
                    char child[4096];
                    snprintf(child, sizeof child, "%s/%s", prefix, name);
                    if (type == FT_DIR) {
                        int sub = -1;
                        if (w_path_open(dirfd, LOOKUP_FOLLOW, name, strlen(name), OF_DIRECTORY, RD, RD, 0, &sub) == 0) {
                            walk_dir(sub, child); w_close(sub);
                        }
                    } else copy_in_file(dirfd, name, child);
                }
            }
            off += sizeof(wdirent) + nl; cookie = next; progressed = 1;
        }
        if (!progressed) break;
        if (used < sizeof dbuf) break;
    }
}

static int preopen_name(int fd, char* name, u32 cap, u8* is_dir) {
    prestat p;
    if (w_prestat_get(fd, &p) != 0) return -1;     // これ以上 preopen 無し
    *is_dir = (p.tag == 0);
    if (p.tag != 0 || p.name_len >= cap) { name[0] = 0; return 0; }
    if (w_prestat_name(fd, name, p.name_len) != 0) return 0;
    name[p.name_len] = 0;
    return 1;
}

__attribute__((constructor))
static void bridge_in(void) {
    int copied = 0;
    for (int fd = 3; fd < 256; fd++) {
        char name[1024]; u8 dir;
        int r = preopen_name(fd, name, sizeof name, &dir);
        if (r < 0) break;
        if (r == 1) { walk_dir(fd, name); copied++; }
    }
    fprintf(stderr, "[bridge] copied %d preopen tree(s) into MEMFS\n", copied);
}

static int find_out(void) {
    for (int fd = 3; fd < 256; fd++) {
        char name[1024]; u8 dir;
        int r = preopen_name(fd, name, sizeof name, &dir);
        if (r < 0) break;
        if (r == 1 && !strcmp(name, "/out")) return fd;
    }
    return -1;
}

static void write_back(int outfd, const char* rel, const char* memfs_path) {
    FILE* r = fopen(memfs_path, "rb");
    if (!r) return;
    int f = -1;
    if (w_path_open(outfd, LOOKUP_FOLLOW, rel, strlen(rel), OF_CREAT | OF_TRUNC, WR, WR, 0, &f) != 0) { fclose(r); return; }
    static char buf[65536]; size_t n;
    while ((n = fread(buf, 1, sizeof buf, r)) > 0) {
        iov v = { buf, (u32)n }; u32 wn = 0;
        w_write(f, &v, 1, &wn);
    }
    fclose(r); w_close(f);
}

static void walk_out(int outfd, const char* base, const char* rel) {
    DIR* d = opendir(base);
    if (!d) return;
    struct dirent* e;
    while ((e = readdir(d))) {
        if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
        char mp[4096], rp[4096];
        snprintf(mp, sizeof mp, "%s/%s", base, e->d_name);
        snprintf(rp, sizeof rp, "%s%s%s", rel, rel[0] ? "/" : "", e->d_name);
        struct stat st;
        if (stat(mp, &st) == 0 && S_ISDIR(st.st_mode)) walk_out(outfd, mp, rp);
        else write_back(outfd, rp, mp);
    }
    closedir(d);
}

__attribute__((destructor))
static void bridge_out(void) {
    int outfd = find_out();
    if (outfd < 0) return;
    walk_out(outfd, "/out", "");
    fprintf(stderr, "[bridge] wrote MEMFS /out back to host preopen\n");
}
