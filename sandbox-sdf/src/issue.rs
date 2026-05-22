use crate::{sdf_circle, sdf_rect, sdf_translate};
use glam::Vec2;

/// issue の金属パネルを正面から見た 2D SDF。
///
/// 外形矩形から大窓4つ・中央小窓・取り付け穴を subtract して構成する。
///
///   ┌──────────────────────┐
///   │  ┌──────┐  ┌──────┐  │  ← 上段窓 2つ
///   │  │      │  │      │  │
///   │  └──────┘  └──────┘  │
///   │       ┌────┐          │  ← 中央小窓
///   │  ┌──────┐  ┌──────┐  │  ← 下段窓 2つ
///   │  │      │  │      │  │
///   │  └──────┘  └──────┘  │
///   └──────────────────────┘
pub fn sdf_issue(p: Vec2) -> f32 {
    // 外形（幅4 × 高さ5、原点中心）
    let body = sdf_rect(p, Vec2::new(-2.0, -2.5), Vec2::new(2.0, 2.5));

    // 大窓 2列×2行
    let win_tl = sdf_rect(p, Vec2::new(-1.7,  0.35), Vec2::new(-0.15,  2.2));
    let win_tr = sdf_rect(p, Vec2::new( 0.15,  0.35), Vec2::new( 1.7,  2.2));
    let win_bl = sdf_rect(p, Vec2::new(-1.7, -2.2 ), Vec2::new(-0.15, -0.35));
    let win_br = sdf_rect(p, Vec2::new( 0.15, -2.2 ), Vec2::new( 1.7, -0.35));

    // 中央小窓（横バーの中）
    let win_c = sdf_rect(p, Vec2::new(-0.35, -0.2), Vec2::new(0.35, 0.2));

    // 取り付け穴（小円）: 大窓の四隅付近に代表点を配置
    const HOLE_R: f32 = 0.1;
    let hole = |cx: f32, cy: f32| sdf_translate(p, Vec2::new(cx, cy), HOLE_R, sdf_circle);
    let holes = [
        hole(-1.85,  2.35), hole( 1.85,  2.35),
        hole(-1.85, -2.35), hole( 1.85, -2.35),
        hole(-1.85,  0.0 ), hole( 1.85,  0.0 ),
        hole( 0.0,   2.35), hole( 0.0,  -2.35),
    ]
    .into_iter()
    .fold(f32::INFINITY, f32::min); // 全穴の和集合

    // 外形から窓・穴を subtract（max(a, -b) で b の内側を除去）
    body.max(-win_tl)
        .max(-win_tr)
        .max(-win_bl)
        .max(-win_br)
        .max(-win_c)
        .max(-holes)
}
